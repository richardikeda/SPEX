use spex_core::types::ProtoSuite;
use spex_mls::{Group, GroupConfig, MlsError, ValidationError};

/// Builds a default protocol suite for secret tests.
fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

/// Ensures member secrets are derived and rotated across commits.
#[test]
fn group_secret_distribution_rotates_on_commit() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x33; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);
    let mut group = Group::create(config).expect("group");

    group.add_member("alice").expect("add alice");
    group.add_member("bob").expect("add bob");

    let initial_secret = group.group_secret().to_vec();
    let initial_distribution = group.distribute_secrets();
    assert_eq!(initial_distribution.len(), 2);

    group.add_member("carol").expect("add carol");

    let rotated_secret = group.group_secret().to_vec();
    let rotated_distribution = group.distribute_secrets();
    assert_eq!(rotated_distribution.len(), 3);
    assert_ne!(initial_secret, rotated_secret);
}

/// Verifies encryption and decryption round-trip for group messages.
#[test]
fn group_encrypt_decrypt_roundtrip() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x44; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);
    let mut group = Group::create(config).expect("group");

    group.add_member("alice").expect("add alice");
    let ciphertext = group
        .encrypt("alice", 42, b"secret payload")
        .expect("encrypt");
    let plaintext = group.decrypt("alice", 42, &ciphertext).expect("decrypt");

    assert_eq!(plaintext, b"secret payload".to_vec());
}

/// Ensures encryption fails for unknown members.
#[test]
fn group_encrypt_rejects_unknown_member() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x55; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);
    let group = Group::create(config).expect("group");

    let error = group.encrypt("mallory", 1, b"oops").expect_err("error");
    assert!(matches!(error, MlsError::UnknownMember(_)));
}

/// Ensures decryption rejects messages with mismatched epoch or configuration metadata.
#[test]
fn group_rejects_messages_with_invalid_epoch_or_config() {
    let proto_suite = test_proto_suite();
    let cfg_hash = vec![0x66; 32];
    let config = GroupConfig::new(proto_suite, 0, 1, cfg_hash);
    let mut group = Group::create(config).expect("group");

    group.add_member("alice").expect("add alice");
    group.add_member("bob").expect("add bob");

    let initial_message = group.encrypt("alice", 1, b"epoch zero").expect("encrypt");

    group.add_member("carol").expect("add carol");

    let epoch_error = group
        .decrypt("alice", 1, &initial_message)
        .expect_err("epoch mismatch");
    assert!(matches!(
        epoch_error,
        MlsError::Validation(ValidationError::EpochMismatch { .. })
    ));

    let current_message = group.encrypt("alice", 2, b"epoch one").expect("encrypt");

    let mut cfg_mismatch = current_message.clone();
    cfg_mismatch.cfg_hash[0] ^= 0xFF;
    let cfg_error = group
        .decrypt("alice", 2, &cfg_mismatch)
        .expect_err("cfg hash mismatch");
    assert!(matches!(
        cfg_error,
        MlsError::Validation(ValidationError::CfgHashMismatch)
    ));

    let mut suite_mismatch = current_message.clone();
    suite_mismatch.proto_suite.ciphersuite_id =
        suite_mismatch.proto_suite.ciphersuite_id.wrapping_add(1);
    let suite_error = group
        .decrypt("alice", 2, &suite_mismatch)
        .expect_err("proto suite mismatch");
    assert!(matches!(
        suite_error,
        MlsError::Validation(ValidationError::ProtoSuiteMismatch)
    ));
}
