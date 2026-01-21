use spex_core::types::ProtoSuite;
use spex_mls::{Group, GroupConfig, GroupMessage, ValidationError};

/// Builds a default protocol suite for membership tests.
fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

/// Ensures group membership commits add/remove members and advance epochs.
#[test]
fn group_membership_commits_update_roster() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x11; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);
    let mut group = Group::create(config).expect("group");

    let add_alice = group.add_member("alice").expect("add alice");
    assert_eq!(add_alice.epoch, 1);
    assert_eq!(add_alice.added_members, vec!["alice".to_string()]);
    assert_eq!(group.members(), &[String::from("alice")]);

    let add_bob = group.add_member("bob").expect("add bob");
    assert_eq!(add_bob.epoch, 2);
    assert_eq!(
        group.members(),
        &[String::from("alice"), String::from("bob")]
    );

    let remove_alice = group.remove_member("alice").expect("remove alice");
    assert_eq!(remove_alice.epoch, 3);
    assert_eq!(remove_alice.removed_members, vec!["alice".to_string()]);
    assert_eq!(group.members(), &[String::from("bob")]);
}

/// Validates cfg_hash/proto_suite matches and rejects epoch/config mismatches.
#[test]
fn group_message_validation_rejects_divergent_metadata() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x22; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash.clone());
    let mut group = Group::create(config).expect("group");
    group.add_member("alice").expect("add alice");

    let valid_message =
        GroupMessage::new(group.epoch(), cfg_hash.clone(), proto_suite, vec![1, 2, 3]);
    assert!(group.validate_message(&valid_message).is_ok());

    let wrong_epoch = GroupMessage::new(group.epoch() + 1, cfg_hash.clone(), proto_suite, vec![]);
    assert!(matches!(
        group.validate_message(&wrong_epoch),
        Err(ValidationError::EpochMismatch { .. })
    ));

    let wrong_cfg_hash = GroupMessage::new(group.epoch(), vec![0xFF; 32], proto_suite, vec![]);
    assert!(matches!(
        group.validate_message(&wrong_cfg_hash),
        Err(ValidationError::CfgHashMismatch)
    ));

    let wrong_suite = GroupMessage::new(
        group.epoch(),
        cfg_hash,
        ProtoSuite {
            major: 9,
            minor: 9,
            ciphersuite_id: 9,
        },
        vec![],
    );
    assert!(matches!(
        group.validate_message(&wrong_suite),
        Err(ValidationError::ProtoSuiteMismatch)
    ));
}
