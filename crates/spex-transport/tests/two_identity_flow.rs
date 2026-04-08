// SPDX-License-Identifier: MPL-2.0
use axum::{extract::Path, routing::get, Json, Router};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};
use serde_json::json;
use spex_core::{
    hash::{hash_bytes, HashId},
    sign::{
        ed25519_sign_hash, ed25519_signing_key_from_seed, ed25519_verify_hash, ed25519_verify_key,
    },
    types::{ContactCard, Ctap2Cbor, Envelope, GrantToken, ProtoSuite, ThreadConfig},
};
use spex_mls::{cfg_hash_for_thread_config, mls_extensions, Commit, Group, GroupConfig};
use spex_transport::chunking::{chunk_data, reassemble_chunks, Chunk, ChunkingConfig};
use spex_transport::inbox::BridgeClient;
use std::collections::{BTreeMap, HashMap};
use tokio::net::TcpListener;

#[derive(Clone)]
struct Identity {
    user_id: Vec<u8>,
    signing_key: ed25519_dalek::SigningKey,
    verifying_key: Vec<u8>,
    device_id: Vec<u8>,
    device_nonce: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestToken {
    from_user_id: String,
    to_user_id: String,
    role: u64,
    created_at: u64,
}

fn build_identity(seed: u8) -> Identity {
    // Builds an identity from a deterministic seed for test repeatability.
    let signing_key = ed25519_signing_key_from_seed(&[seed; 32]).expect("seed");
    let verifying_key = ed25519_verify_key(&signing_key).to_bytes().to_vec();
    let user_id = hash_bytes(HashId::Sha256, &[seed; 16]);
    let device_id = hash_bytes(HashId::Sha256, &[seed; 8])[..16].to_vec();
    let device_nonce = hash_bytes(HashId::Sha256, &[seed; 24])[..16].to_vec();
    Identity {
        user_id,
        signing_key,
        verifying_key,
        device_id,
        device_nonce,
    }
}

fn sign_contact_card(identity: &Identity, card: &ContactCard) -> Vec<u8> {
    // Signs a contact card using the identity's signing key.
    let payload = card.to_ctap2_canonical_bytes().expect("card cbor");
    let digest = hash_bytes(HashId::Sha256, &payload);
    ed25519_sign_hash(&identity.signing_key, &digest)
        .to_bytes()
        .to_vec()
}

fn build_contact_card(identity: &Identity) -> ContactCard {
    // Constructs a signed contact card for the given identity.
    let mut card = ContactCard {
        user_id: identity.user_id.clone(),
        verifying_key: identity.verifying_key.clone(),
        device_id: identity.device_id.clone(),
        device_nonce: identity.device_nonce.clone(),
        issued_at: 1_700_000_000,
        invite: None,
        signature: None,
        extensions: BTreeMap::new(),
    };
    let signature = sign_contact_card(identity, &card);
    card.signature = Some(signature);
    card
}

fn parse_contact_card(bytes: &[u8]) -> Option<ContactCard> {
    // Parses a CBOR contact card from bytes into a ContactCard struct.
    let value: ciborium::Value = ciborium::de::from_reader(bytes).ok()?;
    let map = match value {
        ciborium::Value::Map(map) => map,
        _ => return None,
    };

    let mut card = ContactCard::default();

    for (key, value) in map {
        let key = match key {
            ciborium::Value::Integer(v) => i128::from(v),
            _ => continue,
        };
        match key {
            0 => card.user_id = expect_bytes(value)?,
            1 => card.verifying_key = expect_bytes(value)?,
            2 => card.device_id = expect_bytes(value)?,
            3 => card.device_nonce = expect_bytes(value)?,
            4 => card.issued_at = expect_u64(value)?,
            6 => card.signature = Some(expect_bytes(value)?),
            _ => {}
        }
    }

    if card.user_id.is_empty() || card.verifying_key.is_empty() {
        return None;
    }

    Some(card)
}

fn expect_bytes(value: ciborium::Value) -> Option<Vec<u8>> {
    // Extracts bytes from a CBOR value when possible.
    match value {
        ciborium::Value::Bytes(bytes) => Some(bytes),
        _ => None,
    }
}

fn expect_u64(value: ciborium::Value) -> Option<u64> {
    // Extracts a u64 from a CBOR integer value when possible.
    match value {
        ciborium::Value::Integer(v) => u64::try_from(i128::from(v)).ok(),
        _ => None,
    }
}

fn build_request_token(
    from_user_id: &[u8],
    to_user_id: &[u8],
    role: u64,
    created_at: u64,
) -> String {
    // Builds a base64-encoded request token for the handshake flow.
    let request = RequestToken {
        from_user_id: hex::encode(from_user_id),
        to_user_id: hex::encode(to_user_id),
        role,
        created_at,
    };
    let request_payload = serde_json::to_vec(&request).expect("request payload");
    BASE64_STANDARD.encode(&request_payload)
}

fn decode_request_token(token: &str) -> RequestToken {
    // Decodes a base64-encoded request token into its JSON payload.
    serde_json::from_slice(&BASE64_STANDARD.decode(token).expect("request decode"))
        .expect("request parse")
}

fn build_grant_token(user_id: &[u8], role: u64) -> String {
    // Builds a base64-encoded grant token for the requester.
    let grant = GrantToken {
        user_id: user_id.to_vec(),
        role,
        flags: None,
        expires_at: None,
        extensions: BTreeMap::new(),
    };
    BASE64_STANDARD.encode(grant.to_ctap2_canonical_bytes().expect("grant"))
}

async fn spawn_bridge_with_items(items: Vec<Vec<u8>>) -> String {
    // Spawns a mock bridge server that returns the provided payloads for inbox scans.
    let encoded_items: Vec<String> = items
        .iter()
        .map(|item| BASE64_STANDARD.encode(item))
        .collect();
    let app = Router::new().route(
        "/inbox/:key",
        get(move |Path(_key): Path<String>| {
            let items = encoded_items.clone();
            async move { Json(json!({ "items": items })) }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    format!("http://{}", addr)
}

#[tokio::test]
async fn two_identities_exchange_cards_handshake_and_mls() {
    // Exercises the identity exchange, request/grant handshake, and MLS extensions.
    let alice = build_identity(1);
    let bob = build_identity(2);

    let alice_card = build_contact_card(&alice);
    let bob_card = build_contact_card(&bob);

    let alice_card_bytes = alice_card.to_ctap2_canonical_bytes().expect("card cbor");
    let bob_card_bytes = bob_card.to_ctap2_canonical_bytes().expect("card cbor");

    let received_alice = parse_contact_card(&alice_card_bytes).expect("alice card");
    let received_bob = parse_contact_card(&bob_card_bytes).expect("bob card");

    let alice_sig: [u8; 64] = received_alice
        .signature
        .clone()
        .expect("signature")
        .try_into()
        .expect("signature size");
    let bob_sig: [u8; 64] = received_bob
        .signature
        .clone()
        .expect("signature")
        .try_into()
        .expect("signature size");

    let mut unsigned_alice = received_alice.clone();
    unsigned_alice.signature = None;
    let digest = hash_bytes(
        HashId::Sha256,
        &unsigned_alice.to_ctap2_canonical_bytes().unwrap(),
    );
    let alice_verify = ed25519_verify_key(&alice.signing_key);
    ed25519_verify_hash(&alice_verify, &digest, &Signature::from_bytes(&alice_sig))
        .expect("alice signature");

    let mut unsigned_bob = received_bob.clone();
    unsigned_bob.signature = None;
    let digest = hash_bytes(
        HashId::Sha256,
        &unsigned_bob.to_ctap2_canonical_bytes().unwrap(),
    );
    let bob_verify = ed25519_verify_key(&bob.signing_key);
    ed25519_verify_hash(&bob_verify, &digest, &Signature::from_bytes(&bob_sig))
        .expect("bob signature");

    let request_token = build_request_token(&alice.user_id, &bob.user_id, 1, 1_700_000_050);
    let decoded_request = decode_request_token(&request_token);
    assert_eq!(decoded_request.to_user_id, hex::encode(&bob.user_id));

    let grant_token = build_grant_token(&alice.user_id, decoded_request.role);
    assert!(!grant_token.is_empty());

    let grant = GrantToken {
        user_id: alice.user_id.clone(),
        role: decoded_request.role,
        flags: None,
        expires_at: None,
        extensions: BTreeMap::new(),
    };
    let thread_id = hash_bytes(HashId::Sha256, b"thread");
    let thread_cfg = ThreadConfig {
        proto_major: 1,
        proto_minor: 1,
        ciphersuite_id: 0x0001,
        flags: 0,
        thread_id: thread_id.clone(),
        grants: vec![grant],
        extensions: BTreeMap::new(),
    };
    let cfg_hash = cfg_hash_for_thread_config(HashId::Sha256, &thread_cfg).expect("cfg hash");
    let proto_suite = ProtoSuite {
        major: 1,
        minor: 1,
        ciphersuite_id: 0x0001,
    };
    let mut group = Group::create(GroupConfig::new(
        proto_suite,
        0,
        HashId::Sha256 as u16,
        cfg_hash.clone(),
    ))
    .expect("group");

    let context = group.context();
    let expected_extensions = mls_extensions(proto_suite, 0, HashId::Sha256 as u16, &cfg_hash);
    assert!(context.extensions().contains(&expected_extensions[0]));
    assert!(context.extensions().contains(&expected_extensions[1]));

    let mut commit = Commit::new(1);
    commit.flags = Some(1);
    let updated_context = group.apply_commit(commit).expect("commit");
    let expected_updated_extensions =
        mls_extensions(proto_suite, 1, HashId::Sha256 as u16, &cfg_hash);
    assert!(updated_context
        .extensions()
        .contains(&expected_updated_extensions[0]));
}

#[tokio::test]
async fn two_identities_send_receive_via_dht_and_bridge() {
    // Exercises chunking, DHT-style retrieval, and bridge inbox polling.
    let alice = build_identity(1);
    let bob = build_identity(2);

    let thread_id = hash_bytes(HashId::Sha256, b"thread");
    let envelope = Envelope {
        thread_id,
        epoch: 1,
        seq: 1,
        sender_user_id: alice.user_id.clone(),
        ciphertext: b"oi bob".to_vec(),
        signature: None,
        extensions: BTreeMap::new(),
    };
    let payload = envelope.to_ctap2_canonical_bytes().expect("envelope");
    let chunk_config = ChunkingConfig {
        chunk_size: 8,
        ..ChunkingConfig::default()
    };
    let chunks = chunk_data(&chunk_config, &payload);
    let mut dht_store: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    for chunk in &chunks {
        dht_store.insert(chunk.hash.clone(), chunk.data.clone());
    }
    let mut received_chunks = Vec::new();
    for chunk in &chunks {
        let data = dht_store.get(&chunk.hash).expect("chunk stored").clone();
        received_chunks.push(Chunk {
            index: chunk.index,
            hash: chunk.hash.clone(),
            data,
        });
    }
    let reassembled = reassemble_chunks(&received_chunks);
    assert_eq!(reassembled, payload);

    let inbox_key = bob.user_id.clone();
    let bridge_url = spawn_bridge_with_items(vec![payload.clone()]).await;

    let client = BridgeClient::new(bridge_url);
    let response = client.scan_inbox(&inbox_key).await.expect("scan");
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0], payload);
}
