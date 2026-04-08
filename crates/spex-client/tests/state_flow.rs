// SPDX-License-Identifier: MPL-2.0
use spex_client::{
    create_contact_card_payload, create_identity, create_identity_in_state,
    create_thread_for_state, redeem_contact_card_to_state, send_thread_message_for_state,
    LocalState,
};

/// Ensures contact redemption detects key changes for the same user identifier.
#[test]
fn test_redeem_contact_card_to_state_detects_key_change() {
    let mut state = LocalState::default();
    let identity = create_identity();
    let first_payload =
        create_contact_card_payload(&identity).expect("first card payload should build");
    let first = redeem_contact_card_to_state(&mut state, &first_payload)
        .expect("first contact redemption should succeed");
    assert!(!first.key_changed);

    let mut rotated = create_identity();
    rotated.user_id_hex = identity.user_id_hex.clone();
    let second_payload =
        create_contact_card_payload(&rotated).expect("second card payload should build");
    let second = redeem_contact_card_to_state(&mut state, &second_payload)
        .expect("second contact redemption should succeed");
    assert!(second.key_changed);
    assert!(second.previous_fingerprint.is_some());
}

/// Ensures sending a thread message updates local state and produces transport payloads.
#[test]
fn test_send_thread_message_for_state_updates_state() {
    let mut state = LocalState::default();
    let _identity = create_identity_in_state(&mut state);
    let thread_id =
        create_thread_for_state(&mut state, vec![]).expect("thread creation should succeed");
    let dispatch = send_thread_message_for_state(&mut state, &thread_id, "hello")
        .expect("thread message should send");
    assert_eq!(dispatch.chunk_count, dispatch.chunks.len());
    assert_eq!(state.transport_outbox.len(), 1);
    let thread_state = state
        .threads
        .get(&thread_id)
        .expect("thread state should exist");
    assert_eq!(thread_state.messages.len(), 1);
}
