// SPDX-License-Identifier: MPL-2.0
use spex_core::types::ProtoSuite;
use spex_mls::{GroupConfig, MlsRsClient};

/// Builds a default protocol suite for MLS-rs integration tests.
fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

/// Ensures MLS-rs group creation includes valid SPEX extensions.
#[test]
fn mls_rs_group_creation_validates_extensions() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x10; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);

    let client = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("client");
    let group = client.create_group(config).expect("group");

    group
        .validate_cfg_hash_proto_suite()
        .expect("extensions valid");
    assert_eq!(group.epoch(), 0);
}

/// Ensures MLS-rs commits add and remove members while advancing epochs.
#[test]
fn mls_rs_commits_update_membership_and_epoch() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x11; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);

    let alice = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("alice client");
    let bob = MlsRsClient::new(proto_suite, b"bob".to_vec()).expect("bob client");
    let mut group = alice.create_group(config).expect("group");

    group.add_member(&bob).expect("add bob");
    assert_eq!(group.epoch(), 1);
    assert_eq!(group.member_identities().len(), 2);

    group.remove_member_by_identity(b"bob").expect("remove bob");
    assert_eq!(group.epoch(), 2);
    assert_eq!(group.member_identities(), vec![b"alice".to_vec()]);
}

/// Confirms MLS-rs commits increment epochs for consecutive additions.
#[test]
fn mls_rs_epoch_changes_on_multiple_commits() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x12; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);

    let alice = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("alice client");
    let bob = MlsRsClient::new(proto_suite, b"bob".to_vec()).expect("bob client");
    let carol = MlsRsClient::new(proto_suite, b"carol".to_vec()).expect("carol client");
    let mut group = alice.create_group(config).expect("group");

    group.add_member(&bob).expect("add bob");
    assert_eq!(group.epoch(), 1);

    group.add_member(&carol).expect("add carol");
    assert_eq!(group.epoch(), 2);
    assert_eq!(group.member_identities().len(), 3);
}
