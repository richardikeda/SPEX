// SPDX-License-Identifier: MPL-2.0
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use spex_client::{
    create_contact_card_payload, create_identity_in_state, create_thread_for_state,
    receive_inbox_messages, redeem_contact_card_to_state, LocalState, ThreadState,
};
use spex_core::hash::HashId;
use spex_core::types::Ctap2Cbor;
use spex_transport::inbox::derive_inbox_scan_key;

fn deliver_envelope_to_client(
    recipient_state: &mut LocalState,
    envelope: &spex_core::types::Envelope,
) {
    let payload = envelope.to_ctap2_canonical_bytes().unwrap();
    let payload_base64 = BASE64.encode(&payload);

    // Recipient needs to find this in their inbox.
    let recipient_id_bytes =
        hex::decode(&recipient_state.identity.as_ref().unwrap().user_id_hex).unwrap();
    let scan = derive_inbox_scan_key(HashId::Sha256, &recipient_id_bytes);
    let key_hex = hex::encode(scan.hashed_key);

    recipient_state
        .p2p_inbox
        .entry(key_hex)
        .or_default()
        .push(payload_base64);
}

#[tokio::test]
async fn test_three_party_conversation() {
    let mut alice_state = LocalState::default();
    let mut bob_state = LocalState::default();
    let mut carol_state = LocalState::default();

    let alice = create_identity_in_state(&mut alice_state);
    let bob = create_identity_in_state(&mut bob_state);
    let carol = create_identity_in_state(&mut carol_state);

    // 1. Exchange Contact Cards
    let alice_card = create_contact_card_payload(&alice).unwrap();
    let bob_card = create_contact_card_payload(&bob).unwrap();
    let carol_card = create_contact_card_payload(&carol).unwrap();

    redeem_contact_card_to_state(&mut alice_state, &bob_card).unwrap();
    redeem_contact_card_to_state(&mut alice_state, &carol_card).unwrap();

    redeem_contact_card_to_state(&mut bob_state, &alice_card).unwrap();
    redeem_contact_card_to_state(&mut bob_state, &carol_card).unwrap();

    redeem_contact_card_to_state(&mut carol_state, &alice_card).unwrap();
    redeem_contact_card_to_state(&mut carol_state, &bob_card).unwrap();

    // 2. Alice creates a thread with Bob and Carol
    let members = vec![
        alice.user_id_hex.clone(),
        bob.user_id_hex.clone(),
        carol.user_id_hex.clone(),
    ];
    let thread_id = create_thread_for_state(&mut alice_state, members.clone()).unwrap();

    // Simulate Welcome/Join by copying thread state manually
    let alice_thread = alice_state.threads.get(&thread_id).unwrap();
    let bob_thread =
        serde_json::from_value::<ThreadState>(serde_json::to_value(alice_thread).unwrap()).unwrap();
    let carol_thread =
        serde_json::from_value::<ThreadState>(serde_json::to_value(alice_thread).unwrap()).unwrap();

    bob_state.threads.insert(thread_id.clone(), bob_thread);
    carol_state.threads.insert(thread_id.clone(), carol_thread);

    // 3. Alice sends a message
    // Use `send_thread_message` directly to get the envelope.
    let alice_thread_state = alice_state.threads.get_mut(&thread_id).unwrap();
    let (envelope, _manifest, _chunks) =
        spex_client::send_thread_message(&alice, alice_thread_state, b"Hello Group!").unwrap();

    // 4. Deliver to Bob and Carol
    deliver_envelope_to_client(&mut bob_state, &envelope);
    deliver_envelope_to_client(&mut carol_state, &envelope);

    // 5. Bob receives
    let bob_seed = hex::decode(&bob.user_id_hex).unwrap();
    let bob_results = receive_inbox_messages(&mut bob_state, &bob_seed, None)
        .await
        .unwrap();
    assert_eq!(bob_results.items.len(), 1);
    assert_eq!(
        String::from_utf8(bob_results.items[0].plaintext.clone()).unwrap(),
        "Hello Group!"
    );

    // 6. Carol receives
    let carol_seed = hex::decode(&carol.user_id_hex).unwrap();
    let carol_results = receive_inbox_messages(&mut carol_state, &carol_seed, None)
        .await
        .unwrap();
    assert_eq!(carol_results.items.len(), 1);
    assert_eq!(
        String::from_utf8(carol_results.items[0].plaintext.clone()).unwrap(),
        "Hello Group!"
    );
}
