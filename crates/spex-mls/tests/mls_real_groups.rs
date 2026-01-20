use spex_core::types::ProtoSuite;
use spex_mls::{mls_rs::group::CommitEffect, GroupConfig, MlsRsClient};

/// Builds a default protocol suite for real-group MLS tests.
fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

/// Ensures MLS groups support multi-member commits, updates, removals, and resync flows.
#[test]
fn mls_rs_real_group_commit_update_resync_flow() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x42; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);

    let alice = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("alice client");
    let bob = MlsRsClient::new(proto_suite, b"bob".to_vec()).expect("bob client");
    let carol = MlsRsClient::new(proto_suite, b"carol".to_vec()).expect("carol client");
    let dave = MlsRsClient::new(proto_suite, b"dave".to_vec()).expect("dave client");

    let mut alice_group = alice.create_group(config).expect("create group");

    let add_bob = alice_group.add_member(&bob).expect("add bob");
    let mut bob_group = bob
        .join_group(&add_bob.welcome_messages[0], add_bob.ratchet_tree.clone())
        .expect("bob join");
    assert_eq!(alice_group.epoch(), 1);
    assert_eq!(bob_group.epoch(), 1);

    let add_carol = alice_group.add_member(&carol).expect("add carol");
    bob_group
        .process_commit_message(add_carol.commit_message.clone())
        .expect("bob process carol commit");
    let mut carol_group = carol
        .join_group(&add_carol.welcome_messages[0], add_carol.ratchet_tree.clone())
        .expect("carol join");
    assert_eq!(alice_group.epoch(), 2);
    assert_eq!(bob_group.epoch(), 2);
    assert_eq!(carol_group.epoch(), 2);

    let update = bob_group.self_update().expect("bob update");
    alice_group
        .process_commit_message(update.commit_message.clone())
        .expect("alice process update");
    carol_group
        .process_commit_message(update.commit_message.clone())
        .expect("carol process update");
    assert!(update.contains_update_path);
    assert_eq!(alice_group.epoch(), 3);
    assert_eq!(bob_group.epoch(), 3);
    assert_eq!(carol_group.epoch(), 3);

    let remove_bob = alice_group
        .remove_member_by_identity(b"bob")
        .expect("remove bob");
    carol_group
        .process_commit_message(remove_bob.commit_message.clone())
        .expect("carol process removal");
    let bob_removal = bob_group
        .process_commit_message(remove_bob.commit_message.clone())
        .expect("bob sees removal");
    assert!(matches!(bob_removal.effect, CommitEffect::Removed { .. }));
    assert_eq!(alice_group.member_identities().len(), 2);
    assert_eq!(carol_group.member_identities().len(), 2);

    let group_info = alice_group
        .group_info_message_allowing_external_commit(false)
        .expect("group info");
    let tree = alice_group.export_tree();
    let (dave_group, external_commit) = dave
        .resync_from_group_info(group_info, Some(tree))
        .expect("dave resync");
    alice_group
        .process_commit_message(external_commit.clone())
        .expect("alice process external commit");
    carol_group
        .process_commit_message(external_commit)
        .expect("carol process external commit");
    assert_eq!(alice_group.member_identities().len(), 3);
    assert_eq!(carol_group.member_identities().len(), 3);
    assert_eq!(dave_group.epoch(), alice_group.epoch());
}
