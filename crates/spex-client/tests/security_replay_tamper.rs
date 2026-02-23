use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use spex_client::{
    create_contact_card_payload, create_identity_in_state, create_thread_for_state,
    decrypt_thread_envelope, receive_transport_messages, redeem_contact_card_to_state,
    stage_transport_delivery_for_members, ClientError, LocalState, ThreadState,
};
use spex_core::{
    hash::{hash_ctap2_cbor_value, HashId},
    sign::{ed25519_sign_hash, ed25519_signing_key_from_seed},
    types::Ctap2Cbor,
};
use spex_transport::transport::ChunkDescriptor;
use spex_transport::{
    chunking::{chunk_data, ChunkingConfig},
    manifest_payload, ChunkManifest,
};

/// Creates two clients, exchanges contact cards, and mirrors a shared thread to the recipient.
fn setup_sender_and_recipient() -> (LocalState, LocalState, String) {
    let mut sender_state = LocalState::default();
    let mut recipient_state = LocalState::default();

    let sender_identity = create_identity_in_state(&mut sender_state);
    let recipient_identity = create_identity_in_state(&mut recipient_state);

    let sender_card = create_contact_card_payload(&sender_identity).expect("sender card");
    let recipient_card = create_contact_card_payload(&recipient_identity).expect("recipient card");

    redeem_contact_card_to_state(&mut sender_state, &recipient_card).expect("sender redeem");
    redeem_contact_card_to_state(&mut recipient_state, &sender_card).expect("recipient redeem");

    let thread_id = create_thread_for_state(
        &mut sender_state,
        vec![
            sender_identity.user_id_hex.clone(),
            recipient_identity.user_id_hex.clone(),
        ],
    )
    .expect("thread creation");

    let sender_thread = sender_state.threads.get(&thread_id).expect("sender thread");
    let mirrored_thread = serde_json::from_value::<ThreadState>(
        serde_json::to_value(sender_thread).expect("to value"),
    )
    .expect("thread clone");
    recipient_state
        .threads
        .insert(thread_id.clone(), mirrored_thread);

    (sender_state, recipient_state, thread_id)
}

/// Signs an envelope after mutating it so tests can isolate decryption invariants from signature checks.
fn resign_envelope(
    sender_state: &LocalState,
    mut envelope: spex_core::types::Envelope,
) -> spex_core::types::Envelope {
    let identity = sender_state.identity.as_ref().expect("sender identity");
    let seed: [u8; 32] = hex::decode(&identity.signing_key_hex)
        .expect("seed hex")
        .try_into()
        .expect("seed length");
    let signing_key = ed25519_signing_key_from_seed(&seed).expect("signing key");

    envelope.signature = None;
    let digest = hash_ctap2_cbor_value(HashId::Sha256, &envelope).expect("envelope digest");
    let signature = ed25519_sign_hash(&signing_key, &digest);
    envelope.signature = Some(signature.to_bytes().to_vec());
    envelope
}

/// Clones thread state through serde to avoid requiring `Clone` on the type.
fn duplicate_thread_state(thread: &ThreadState) -> ThreadState {
    serde_json::from_value(serde_json::to_value(thread).expect("thread to value"))
        .expect("thread from value")
}

/// Ensures replaying valid transport material in different times still decrypts deterministically.
#[test]
fn replay_valid_transport_material_is_deterministic() {
    let (mut sender_state, mut recipient_state, thread_id) = setup_sender_and_recipient();
    let sender_identity = sender_state.identity.clone().expect("sender identity");

    let large_payload = vec![b'A'; 96 * 1024];
    let thread_state = sender_state
        .threads
        .get_mut(&thread_id)
        .expect("sender thread state");
    let (_envelope, manifest, chunks) =
        spex_client::send_thread_message(&sender_identity, thread_state, &large_payload)
            .expect("send thread message");

    let recipient_user = recipient_state
        .identity
        .as_ref()
        .expect("recipient identity")
        .user_id_hex
        .clone();
    stage_transport_delivery_for_members(
        &mut recipient_state,
        std::slice::from_ref(&recipient_user),
        &sender_identity.user_id_hex,
        &manifest,
        &chunks,
    )
    .expect("stage transport 1");

    let recipient_seed = hex::decode(&recipient_user).expect("recipient user decode");
    let first =
        receive_transport_messages(&mut recipient_state, &recipient_seed).expect("receive first");
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].plaintext, large_payload);

    stage_transport_delivery_for_members(
        &mut recipient_state,
        std::slice::from_ref(&recipient_user),
        &sender_identity.user_id_hex,
        &manifest,
        &chunks,
    )
    .expect("stage transport replay");
    let second =
        receive_transport_messages(&mut recipient_state, &recipient_seed).expect("receive replay");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].plaintext, large_payload);
}

/// Ensures tampering across chunk hash, manifest structure, and envelope signature is rejected.
#[test]
fn transport_tampering_is_rejected_and_does_not_mutate_local_messages() {
    let (mut sender_state, mut recipient_state, thread_id) = setup_sender_and_recipient();
    let sender_identity = sender_state.identity.clone().expect("sender identity");
    let recipient_user = recipient_state
        .identity
        .as_ref()
        .expect("recipient identity")
        .user_id_hex
        .clone();
    let recipient_seed = hex::decode(&recipient_user).expect("recipient user decode");

    let before_len = recipient_state
        .threads
        .get(&thread_id)
        .expect("recipient thread")
        .messages
        .len();

    let thread_state = sender_state
        .threads
        .get_mut(&thread_id)
        .expect("sender thread state");
    let (envelope, manifest, chunks) =
        spex_client::send_thread_message(&sender_identity, thread_state, &vec![b'Z'; 96 * 1024])
            .expect("send thread message");

    stage_transport_delivery_for_members(
        &mut recipient_state,
        std::slice::from_ref(&recipient_user),
        &sender_identity.user_id_hex,
        &manifest,
        &chunks,
    )
    .expect("stage valid transport");

    let first_hash_hex = hex::encode(&manifest.chunks[0].hash);
    recipient_state.transport_chunk_store.insert(
        first_hash_hex,
        BASE64_STANDARD.encode(b"invalid chunk bytes"),
    );

    let hash_err = receive_transport_messages(&mut recipient_state, &recipient_seed)
        .expect_err("tampered chunk must fail");
    assert!(
        matches!(hash_err, ClientError::Transport(message) if message.contains("chunk hash mismatch"))
    );

    let partial_manifest = ChunkManifest {
        chunks: manifest.chunks[..manifest.chunks.len().saturating_sub(1)].to_vec(),
        total_len: manifest.total_len,
    };
    let partial_payload = manifest_payload(&partial_manifest).expect("partial payload");
    recipient_state.transport_gossip.clear();
    recipient_state.transport_gossip.insert(
        hex::encode(
            spex_transport::inbox::derive_inbox_scan_key(HashId::Sha256, &recipient_seed)
                .hashed_key,
        ),
        vec![BASE64_STANDARD.encode(partial_payload)],
    );
    for chunk in &chunks {
        recipient_state.transport_chunk_store.insert(
            hex::encode(&chunk.hash),
            BASE64_STANDARD.encode(&chunk.data),
        );
    }

    let partial_err = receive_transport_messages(&mut recipient_state, &recipient_seed)
        .expect_err("partial manifest must fail");
    assert!(
        matches!(partial_err, ClientError::Transport(message) if message.contains("payload length mismatch"))
    );

    let mut tampered_envelope = envelope.clone();
    let signature = tampered_envelope.signature.as_mut().expect("signature");
    signature[0] ^= 0x01;
    let tampered_payload = tampered_envelope
        .to_ctap2_canonical_bytes()
        .expect("tampered envelope bytes");
    let tampered_chunks = chunk_data(&ChunkingConfig::default(), &tampered_payload);
    let tampered_manifest = ChunkManifest {
        chunks: tampered_chunks
            .iter()
            .map(|chunk| ChunkDescriptor {
                index: chunk.index,
                hash: chunk.hash.clone(),
            })
            .collect(),
        total_len: tampered_payload.len(),
    };

    stage_transport_delivery_for_members(
        &mut recipient_state,
        std::slice::from_ref(&recipient_user),
        &sender_identity.user_id_hex,
        &tampered_manifest,
        &tampered_chunks,
    )
    .expect("stage signature tamper");
    let sig_err = receive_transport_messages(&mut recipient_state, &recipient_seed)
        .expect_err("signature tamper must fail");
    assert!(matches!(sig_err, ClientError::SignatureInvalid));

    let after_len = recipient_state
        .threads
        .get(&thread_id)
        .expect("recipient thread")
        .messages
        .len();
    assert_eq!(before_len, after_len);
}

/// Ensures context-mixing attempts are rejected for thread_id, cfg_hash, and epoch invariants.
#[test]
fn context_mixing_rejected_by_thread_cfg_hash_and_epoch_invariants() {
    let (mut sender_state, recipient_state, thread_id) = setup_sender_and_recipient();
    let sender_identity = sender_state.identity.clone().expect("sender identity");
    let sender_thread = sender_state
        .threads
        .get_mut(&thread_id)
        .expect("sender thread state");
    let (envelope, _manifest, _chunks) =
        spex_client::send_thread_message(&sender_identity, sender_thread, b"context mix target")
            .expect("send thread message");

    let base_thread = duplicate_thread_state(
        recipient_state
            .threads
            .get(&thread_id)
            .expect("recipient thread"),
    );

    let mut wrong_thread_state = duplicate_thread_state(&base_thread);
    wrong_thread_state.thread_id_hex = hex::encode([9u8; 32]);
    let thread_err = decrypt_thread_envelope(&recipient_state, &wrong_thread_state, &envelope)
        .expect_err("thread mismatch must fail");
    assert!(matches!(thread_err, ClientError::ThreadNotFound));

    let mut wrong_cfg_state = duplicate_thread_state(&base_thread);
    wrong_cfg_state.cfg_hash_hex = hex::encode([4u8; 32]);
    let cfg_err = decrypt_thread_envelope(&recipient_state, &wrong_cfg_state, &envelope)
        .expect_err("cfg_hash mismatch must fail");
    assert!(matches!(cfg_err, ClientError::Crypto(_)));

    let mut tampered_epoch = envelope.clone();
    tampered_epoch.epoch = tampered_epoch.epoch.saturating_add(7);
    tampered_epoch = resign_envelope(&sender_state, tampered_epoch);
    let epoch_err = decrypt_thread_envelope(&recipient_state, &base_thread, &tampered_epoch)
        .expect_err("epoch mismatch must fail");
    assert!(matches!(epoch_err, ClientError::Crypto(_)));
}
