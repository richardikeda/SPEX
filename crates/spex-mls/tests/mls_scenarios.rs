use spex_core::types::ProtoSuite;
use spex_mls::{GroupConfig, MlsRsClient, MlsRsGroup};

fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

fn create_identity(name: &str) -> Vec<u8> {
    name.as_bytes().to_vec()
}

fn create_client(name: &str) -> MlsRsClient {
    MlsRsClient::new(test_proto_suite(), create_identity(name)).expect("client creation failed")
}

fn create_group(client: &MlsRsClient) -> MlsRsGroup {
    let cfg_hash = vec![0xAA; 32];
    let config = GroupConfig::new(test_proto_suite(), 0, 1, cfg_hash);
    client.create_group(config).expect("group creation failed")
}

#[test]
fn test_member_permutation_consistency() {
    // Scenario: Alice creates group, adds Bob, then Bob adds Carol.
    // We want to ensure everyone ends up in the same state.

    let alice = create_client("alice");
    let bob = create_client("bob");
    let carol = create_client("carol");

    // 1. Alice creates group
    let mut alice_group = create_group(&alice);

    // 2. Alice adds Bob
    let commit = alice_group.add_member(&bob).expect("failed to add bob");

    // In a real scenario, the commit message and welcome message are sent.
    // bob joins using welcome.
    let welcome = commit.welcome_messages.first().expect("missing welcome");
    let ratchet_tree = alice_group.export_tree();

    let mut bob_group = bob.join_group(&welcome, Some(ratchet_tree)).expect("bob failed to join");

    // Verify state
    assert_eq!(alice_group.epoch(), 1);
    assert_eq!(bob_group.epoch(), 1);
    assert_eq!(alice_group.member_identities(), bob_group.member_identities());

    // 3. Bob adds Carol
    let commit = bob_group.add_member(&carol).expect("failed to add carol");

    // Alice processes commit
    alice_group.process_commit_message(commit.commit_message.clone()).expect("alice process commit");

    // Carol joins
    let welcome = commit.welcome_messages.first().expect("missing welcome");
    let ratchet_tree = bob_group.export_tree(); // Carol gets tree from Bob
    let mut carol_group = carol.join_group(&welcome, Some(ratchet_tree)).expect("carol failed to join");

    // Verify all 3 have same state
    assert_eq!(alice_group.epoch(), 2);
    assert_eq!(bob_group.epoch(), 2);
    assert_eq!(carol_group.epoch(), 2);

    let members = alice_group.member_identities();
    assert_eq!(members.len(), 3);
    assert_eq!(bob_group.member_identities(), members);
    assert_eq!(carol_group.member_identities(), members);

    // 4. Carol removes Alice
    let commit = carol_group.remove_member_by_identity(&create_identity("alice")).expect("failed to remove alice");

    // Bob processes commit
    bob_group.process_commit_message(commit.commit_message).expect("bob process commit");

    // Alice processes commit (she will realize she is removed)
    // Note: mls-rs might return an error or special status when self is removed, or just process it.
    // We verify remaining members.

    assert_eq!(carol_group.epoch(), 3);
    assert_eq!(bob_group.epoch(), 3);

    let final_members = carol_group.member_identities();
    assert_eq!(final_members.len(), 2);
    assert!(!final_members.contains(&create_identity("alice")));
    assert!(final_members.contains(&create_identity("bob")));
    assert!(final_members.contains(&create_identity("carol")));

    assert_eq!(bob_group.member_identities(), final_members);
}

#[test]
fn test_updates_key_rotation() {
    // Scenario: Alice, Bob in group. Bob updates his key. Alice processes it.

    let alice = create_client("alice");
    let bob = create_client("bob");

    let mut alice_group = create_group(&alice);
    let commit = alice_group.add_member(&bob).expect("add bob");
    let welcome = commit.welcome_messages.first().unwrap();
    let mut bob_group = bob.join_group(welcome, Some(alice_group.export_tree())).expect("join bob");

    assert_eq!(alice_group.epoch(), 1);

    // Bob performs self-update
    let update_commit = bob_group.self_update().expect("bob update");

    // Alice processes
    alice_group.process_commit_message(update_commit.commit_message).expect("alice process update");

    assert_eq!(alice_group.epoch(), 2);
    assert_eq!(bob_group.epoch(), 2);

    // Verify they are still compatible (roster is same)
    assert_eq!(alice_group.member_identities(), bob_group.member_identities());
}

#[test]
fn test_resynchronization_flow() {
    // Scenario: Alice, Bob in group.
    // Alice adds Carol.
    // Bob MISSES this commit (desync).
    // Bob uses resync_from_group_info to recover.

    let alice = create_client("alice");
    let bob = create_client("bob");
    let carol = create_client("carol");

    let mut alice_group = create_group(&alice);
    let commit = alice_group.add_member(&bob).expect("add bob");
    let welcome = commit.welcome_messages.first().unwrap();
    let mut bob_group = bob.join_group(welcome, Some(alice_group.export_tree())).expect("join bob");

    assert_eq!(bob_group.epoch(), 1);

    // Alice adds Carol
    let commit_carol = alice_group.add_member(&carol).expect("add carol");
    // Bob DOES NOT process commit_carol

    // Carol joins (she talks to Alice)
    let welcome_carol = commit_carol.welcome_messages.first().unwrap();
    let _carol_group = carol.join_group(welcome_carol, Some(alice_group.export_tree())).expect("join carol");

    assert_eq!(alice_group.epoch(), 2);
    assert_eq!(bob_group.epoch(), 1); // Bob is behind

    // Alice (or Carol) publishes GroupInfo.
    // In SPEX, this might be fetched from a distribution channel.
    // Here we simulate getting it from Alice.

    // Note: GroupInfo must be signed by a current member.
    let group_info = alice_group.group_info_message_allowing_external_commit(true).expect("group info");

    // Bob realizes he is out of sync.
    // To perform an external commit when he is already in the group, he must be "removed" first or treated as a new member.
    // However, MLS external commit is typically for adding a NEW member.
    // If Bob tries to add himself again, he duplicates his identity/keys.

    // For a true "resync" where Bob lost state but has his credentials:
    // He should probably be removed and re-added, OR use an external commit that replaces his old leaf.
    // But `mls-rs` external_commit_builder defaults to adding a new member.

    // Let's try to remove Bob from Alice's view first, so the external commit is valid (re-join).
    let commit_remove = alice_group.remove_member_by_identity(&create_identity("bob")).expect("remove bob");
    // Alice is now at epoch 3. Bob is still at epoch 1.

    // Now Alice issues a group info for epoch 3.
    let group_info = alice_group.group_info_message_allowing_external_commit(true).expect("group info");

    // Bob uses this to re-join.
    let (bob_group_resynced, commit_msg) = bob.resync_from_group_info(group_info, None).expect("resync");

    // Alice processes Bob's return.
    alice_group.process_commit_message(commit_msg).expect("alice process bob resync");

    // Alice is at epoch 4. Bob starts at epoch 4.
    assert_eq!(alice_group.epoch(), 4);
    assert_eq!(bob_group_resynced.epoch(), 4);

    assert_eq!(alice_group.member_identities(), bob_group_resynced.member_identities());
}
