// SPDX-License-Identifier: MPL-2.0
use spex_client::{
    create_identity, create_thread_state, decrypt_thread_envelope, fingerprint_hex, now_unix,
    send_thread_message, ContactState, LocalState,
};

/// Ensures inbox envelope decryption succeeds with valid signatures and membership.
#[test]
fn decrypts_envelope_payload_for_member() {
    let alice = create_identity();
    let bob = create_identity();
    let bob_user_id_hex = bob.user_id_hex.clone();

    let mut thread_state =
        create_thread_state(&alice, vec![bob_user_id_hex]).expect("thread state");
    let (envelope, _manifest, _chunks) =
        send_thread_message(&alice, &mut thread_state, b"hello bob").expect("send");

    let mut bob_state = LocalState {
        identity: Some(bob),
        ..Default::default()
    };
    let verifying_key_bytes = hex::decode(&alice.verifying_key_hex).expect("verify key");
    bob_state.contacts.insert(
        alice.user_id_hex.clone(),
        ContactState {
            user_id_hex: alice.user_id_hex.clone(),
            verifying_key_hex: alice.verifying_key_hex.clone(),
            fingerprint: fingerprint_hex(&verifying_key_bytes),
            device_id_hex: alice.device_id_hex.clone(),
            last_seen_at: now_unix(),
        },
    );

    let plaintext = decrypt_thread_envelope(&bob_state, &thread_state, &envelope).expect("decrypt");
    assert_eq!(plaintext, b"hello bob");
}
