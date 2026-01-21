use axum::{extract::Path, routing::get, Json, Router};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::json;
use spex_client::{
    create_checkpoint_entry, create_contact_card_payload, create_identity, create_recovery_entry,
    create_request_payload, create_revocation_entry, create_thread_state, decrypt_thread_envelope,
    fingerprint_hex, load_checkpoint_log, log_consistency, publish_thread_message_transport,
    receive_inbox_messages, receive_transport_messages, redeem_contact_card_payload,
    save_checkpoint_log, send_thread_message, sign_grant, stage_p2p_inbox_delivery,
    stage_transport_delivery, validate_request_puzzle, validate_signed_grant, ClientError,
    ContactState, LocalState, RequestToken, SignedGrantToken,
};
use spex_core::{
    hash::{hash_ctap2_cbor_value, HashId},
    log::{CheckpointEntry, CheckpointLog, LogConsistency},
    pow,
    sign::ed25519_verify_hash,
    types::{Ctap2Cbor, GrantToken},
};
use spex_transport::inbox::{BridgeClient, InboxSource};
use tokio::net::TcpListener;

/// Builds a contact state entry from a redeemed contact card.
fn contact_state_from_card(card: &spex_core::types::ContactCard) -> ContactState {
    let fingerprint = fingerprint_hex(&card.verifying_key);
    ContactState {
        user_id_hex: hex::encode(&card.user_id),
        verifying_key_hex: hex::encode(&card.verifying_key),
        fingerprint,
        device_id_hex: hex::encode(&card.device_id),
        last_seen_at: spex_client::now_unix(),
    }
}

/// Clones a thread state by copying all persisted fields.
fn clone_thread_state(thread: &spex_client::ThreadState) -> spex_client::ThreadState {
    let messages = thread
        .messages
        .iter()
        .map(|message| spex_client::MessageState {
            sender_user_id: message.sender_user_id.clone(),
            text: message.text.clone(),
            sent_at: message.sent_at,
        })
        .collect();
    spex_client::ThreadState {
        thread_id_hex: thread.thread_id_hex.clone(),
        members: thread.members.clone(),
        created_at: thread.created_at,
        messages,
        proto_major: thread.proto_major,
        proto_minor: thread.proto_minor,
        ciphersuite_id: thread.ciphersuite_id,
        cfg_hash_id: thread.cfg_hash_id,
        cfg_hash_hex: thread.cfg_hash_hex.clone(),
        flags: thread.flags,
        initial_secret_hex: thread.initial_secret_hex.clone(),
        next_seq: thread.next_seq,
        epoch: thread.epoch,
    }
}

/// Builds a PoW puzzle token with explicit parameters for request validation tests.
fn build_puzzle_token(recipient_key: &[u8], params: pow::PowParams) -> spex_client::PuzzleToken {
    let nonce = pow::generate_pow_nonce(pow::PowNonceParams::default());
    let puzzle_input = pow::build_puzzle_input(&nonce, recipient_key);
    let puzzle_output =
        pow::generate_puzzle_output(recipient_key, &puzzle_input, params).expect("puzzle output");
    spex_client::PuzzleToken {
        recipient_key: BASE64_STANDARD.encode(recipient_key),
        puzzle_input: BASE64_STANDARD.encode(puzzle_input),
        puzzle_output: BASE64_STANDARD.encode(puzzle_output),
        params: spex_client::PowParamsPayload {
            memory_kib: params.memory_kib,
            iterations: params.iterations,
            parallelism: params.parallelism,
            output_len: params.output_len,
        },
    }
}

/// Verifies a signed grant token against the grant payload.
fn verify_signed_grant(request: &RequestToken, signed: &SignedGrantToken) {
    let user_id = BASE64_STANDARD
        .decode(signed.user_id.as_bytes())
        .expect("grant user id");
    let verifying_key_bytes = BASE64_STANDARD
        .decode(signed.verifying_key.as_bytes())
        .expect("grant verifying key");
    let signature_bytes = BASE64_STANDARD
        .decode(signed.signature.as_bytes())
        .expect("grant signature");
    let signature_array: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .expect("signature size");
    let verifying_key = VerifyingKey::from_bytes(&verifying_key_bytes.try_into().expect("key"))
        .expect("verify key");
    let signature = Signature::from_bytes(&signature_array);
    let grant = GrantToken {
        user_id,
        role: request.role,
        flags: None,
        expires_at: None,
        extensions: Default::default(),
    };
    let hash = hash_ctap2_cbor_value(HashId::Sha256, &grant).expect("grant hash");
    ed25519_verify_hash(&verifying_key, &hash, &signature).expect("grant signature valid");
}

/// Spawns a mock bridge server that returns the provided payloads for inbox scans.
async fn spawn_bridge_with_items(items: Vec<Vec<u8>>) -> String {
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

/// Exercises card exchange, request/grant with PoW, MLS thread setup, and bridge/P2P delivery.
#[tokio::test]
async fn e2e_two_identities_card_request_grant_pow_mls_bridge_p2p() {
    let alice_identity = create_identity();
    let alice_user_id_hex = alice_identity.user_id_hex.clone();
    let bob_identity = create_identity();
    let bob_user_id_hex = bob_identity.user_id_hex.clone();

    let alice_card_payload = create_contact_card_payload(&alice_identity).expect("alice card");
    let bob_card_payload = create_contact_card_payload(&bob_identity).expect("bob card");

    let alice_card = redeem_contact_card_payload(&alice_card_payload).expect("alice redeem");
    let bob_card = redeem_contact_card_payload(&bob_card_payload).expect("bob redeem");

    assert_eq!(hex::encode(&alice_card.user_id), alice_user_id_hex);
    assert_eq!(hex::encode(&bob_card.user_id), bob_user_id_hex);

    let (request, request_payload) =
        create_request_payload(&alice_identity, &bob_user_id_hex, 1).expect("request");
    let puzzle = request.puzzle.as_ref().expect("puzzle");
    validate_request_puzzle(puzzle, &hex::decode(&bob_user_id_hex).expect("bob id"))
        .expect("puzzle valid");

    let (_request_token, signed_grant) =
        spex_client::accept_request_payload(&bob_identity, &request_payload).expect("grant");
    verify_signed_grant(&request, &signed_grant);

    let mut thread_state =
        create_thread_state(&alice_identity, vec![bob_user_id_hex.clone()]).expect("thread");

    let mut bob_state = LocalState::default();
    bob_state.identity = Some(bob_identity);
    bob_state.contacts.insert(
        alice_user_id_hex.clone(),
        contact_state_from_card(&alice_card),
    );
    bob_state.threads.insert(
        thread_state.thread_id_hex.clone(),
        clone_thread_state(&thread_state),
    );

    let (envelope, _manifest, _chunks) =
        send_thread_message(&alice_identity, &mut thread_state, b"hello bob")
            .expect("send message");
    stage_p2p_inbox_delivery(&mut bob_state, &thread_state, &alice_user_id_hex, &envelope)
        .expect("stage inbox");

    let inbox_key = hex::decode(&bob_user_id_hex).expect("bob key");
    let response = receive_inbox_messages(&mut bob_state, &inbox_key, None)
        .await
        .expect("receive p2p");
    assert!(matches!(response.source, InboxSource::Kademlia));
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].plaintext, b"hello bob");

    let (bridge_envelope, _manifest, _chunks) =
        send_thread_message(&alice_identity, &mut thread_state, b"hello bridge")
            .expect("send bridge");
    let bridge_payload = bridge_envelope
        .to_ctap2_canonical_bytes()
        .expect("bridge payload");
    let bridge_url = spawn_bridge_with_items(vec![bridge_payload.clone()]).await;
    let bridge_client = BridgeClient::new(bridge_url);

    let response = receive_inbox_messages(&mut bob_state, &inbox_key, Some(&bridge_client))
        .await
        .expect("receive bridge");
    assert!(matches!(response.source, InboxSource::Bridge));
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].plaintext, b"hello bridge");
}

/// Exercises transport manifest publication, chunk recovery, and envelope decryption.
#[test]
fn e2e_two_identities_transport_publish_recover_and_decrypt() {
    let alice_identity = create_identity();
    let alice_user_id_hex = alice_identity.user_id_hex.clone();
    let bob_identity = create_identity();
    let bob_user_id_hex = bob_identity.user_id_hex.clone();

    let mut thread_state =
        create_thread_state(&alice_identity, vec![bob_user_id_hex.clone()]).expect("thread");
    let alice_card_payload = create_contact_card_payload(&alice_identity).expect("card");
    let alice_card = redeem_contact_card_payload(&alice_card_payload).expect("redeem");

    let mut bob_state = LocalState::default();
    bob_state.identity = Some(bob_identity);
    bob_state.contacts.insert(
        alice_user_id_hex.clone(),
        contact_state_from_card(&alice_card),
    );
    bob_state.threads.insert(
        thread_state.thread_id_hex.clone(),
        clone_thread_state(&thread_state),
    );

    let (_envelope, manifest, chunks, _outbox_item) =
        publish_thread_message_transport(&alice_identity, &mut thread_state, b"hello transport")
            .expect("publish transport");

    stage_transport_delivery(
        &mut bob_state,
        &thread_state,
        &alice_user_id_hex,
        &manifest,
        &chunks,
    )
    .expect("stage transport");

    let inbox_key = hex::decode(&bob_user_id_hex).expect("bob key");
    let response =
        receive_transport_messages(&mut bob_state, &inbox_key).expect("receive transport");
    assert!(matches!(response.source, InboxSource::Kademlia));
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].plaintext, b"hello transport");
}

/// Ensures request puzzles at the minimum PoW parameters validate successfully.
#[test]
fn accepts_minimum_pow_request_puzzle() {
    let recipient_key = b"recipient";
    let puzzle = build_puzzle_token(recipient_key, pow::PowParams::minimum());

    validate_request_puzzle(&puzzle, recipient_key).expect("minimum pow valid");
}

/// Ensures weak PoW parameters are rejected during P2P request validation.
#[test]
fn rejects_weak_pow_request_puzzle() {
    let recipient_key = b"recipient";
    let weak_params = pow::PowParams {
        memory_kib: 8 * 1024,
        iterations: 1,
        parallelism: 1,
        output_len: 32,
    };
    let puzzle = build_puzzle_token(recipient_key, weak_params);

    let result = validate_request_puzzle(&puzzle, recipient_key);
    assert!(matches!(result, Err(ClientError::InvalidPuzzle)));
}

/// Ensures invalid grant signatures are rejected during P2P grant validation.
#[test]
fn rejects_invalid_grant_signature_in_p2p_validation() {
    let identity = create_identity();
    let grant = GrantToken {
        user_id: hex::decode(&identity.user_id_hex).expect("user id"),
        role: 1,
        flags: None,
        expires_at: None,
        extensions: Default::default(),
    };
    let mut signed = sign_grant(&identity, &grant).expect("signed grant");
    signed.signature = BASE64_STANDARD.encode([8u8; 64]);

    let result = validate_signed_grant(&signed, 1_700_000_000);
    assert!(matches!(result, Err(ClientError::GrantInvalid)));
}

/// Ensures expired grants are rejected during P2P grant validation.
#[test]
fn rejects_expired_grant_in_p2p_validation() {
    let identity = create_identity();
    let now = 1_700_000_000;
    let grant = GrantToken {
        user_id: hex::decode(&identity.user_id_hex).expect("user id"),
        role: 1,
        flags: None,
        expires_at: Some(now - 1),
        extensions: Default::default(),
    };
    let signed = sign_grant(&identity, &grant).expect("signed grant");

    let result = validate_signed_grant(&signed, now);
    assert!(matches!(result, Err(ClientError::GrantExpired)));
}

/// Verifies revocation entries persist and log consistency checks behave as expected.
#[test]
fn revocation_entries_roundtrip_and_log_consistency() {
    let identity = create_identity();
    let revoked_key_hex = identity.verifying_key_hex.clone();

    let mut log = CheckpointLog::new();
    let checkpoint = create_checkpoint_entry(&identity).expect("checkpoint");
    log.append(CheckpointEntry::Key(checkpoint))
        .expect("append key");

    let revocation = create_revocation_entry(
        &identity,
        &revoked_key_hex,
        None,
        Some("compromised".to_string()),
    )
    .expect("revocation");
    log.append(CheckpointEntry::Revocation(revocation))
        .expect("append revocation");

    let mut state = LocalState::default();
    save_checkpoint_log(&mut state, &log).expect("save log");
    let restored = load_checkpoint_log(&state).expect("load log");
    assert_eq!(log, restored);

    let mut remote = restored.clone();
    let recovery = create_recovery_entry(&identity).expect("recovery");
    remote
        .append(CheckpointEntry::Recovery(recovery))
        .expect("append recovery");

    assert_eq!(log_consistency(&log, &remote), LogConsistency::LocalBehind);
}

/// Confirms envelope decryption succeeds with a trusted contact entry.
#[test]
fn decrypts_envelope_with_contact_state() {
    let alice_identity = create_identity();
    let bob_identity = create_identity();
    let bob_user_id_hex = bob_identity.user_id_hex.clone();

    let mut thread_state =
        create_thread_state(&alice_identity, vec![bob_user_id_hex]).expect("thread");
    let (envelope, _manifest, _chunks) =
        send_thread_message(&alice_identity, &mut thread_state, b"hello").expect("send");

    let mut bob_state = LocalState::default();
    bob_state.identity = Some(bob_identity);
    let alice_card_payload = create_contact_card_payload(&alice_identity).expect("card");
    let alice_card = redeem_contact_card_payload(&alice_card_payload).expect("redeem");
    bob_state.contacts.insert(
        alice_identity.user_id_hex.clone(),
        contact_state_from_card(&alice_card),
    );
    bob_state.threads.insert(
        thread_state.thread_id_hex.clone(),
        clone_thread_state(&thread_state),
    );

    let plaintext = decrypt_thread_envelope(&bob_state, &thread_state, &envelope).expect("decrypt");
    assert_eq!(plaintext, b"hello");
}
