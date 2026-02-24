use spex_core::types::ProtoSuite;
use spex_mls::{
    parse_external_commit, Commit, ExternalCommit, Group, GroupConfig, GroupMessage, MlsError,
    MlsRsClient, MlsRsError, ValidationError,
};

/// Clones a commit into an explicit external commit carrying the expected epoch number.
fn explicit_external(epoch: u64, commit_message: &mls_rs::MlsMessage) -> ExternalCommit {
    ExternalCommit {
        epoch,
        message: commit_message.clone(),
    }
}

/// Builds a default protocol suite for deterministic MLS integration tests.
fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

/// Verifies concurrent add/remove commits in the same interval are rejected deterministically.
#[test]
fn concurrent_add_and_remove_same_interval_is_rejected() {
    let proto_suite = test_proto_suite();
    let config = GroupConfig::new(proto_suite, 0, 1, vec![0x22; 32]);

    let alice = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("alice client");
    let bob = MlsRsClient::new(proto_suite, b"bob".to_vec()).expect("bob client");
    let carol = MlsRsClient::new(proto_suite, b"carol".to_vec()).expect("carol client");
    let dave = MlsRsClient::new(proto_suite, b"dave".to_vec()).expect("dave client");

    let mut alice_group = alice.create_group(config).expect("create group");
    let add_bob = alice_group.add_member(&bob).expect("add bob");
    let mut bob_group = bob
        .join_group(&add_bob.welcome_messages[0], add_bob.ratchet_tree.clone())
        .expect("bob join");

    let add_carol = alice_group.add_member(&carol).expect("add carol");
    bob_group
        .process_commit_message(add_carol.commit_message.clone())
        .expect("bob process add carol");

    let add_dave = alice_group.add_member(&dave).expect("alice add dave");
    let remove_carol = bob_group
        .remove_member_by_identity(b"carol")
        .expect("bob remove carol");

    let err = alice_group
        .process_external_commit_explicit(ExternalCommit {
            epoch: 3,
            message: remove_carol.commit_message,
        })
        .expect_err("stale epoch should be rejected");
    assert!(matches!(err, MlsRsError::OutOfOrderCommit(_)));

    assert_eq!(add_dave.welcome_messages.len(), 1);
}

/// Verifies resynchronization can recover when epoch N+1 is received before epoch N.
#[test]
fn receives_n_plus_one_without_n_then_recovers() {
    let proto_suite = test_proto_suite();
    let config = GroupConfig::new(proto_suite, 0, 1, vec![0x33; 32]);

    let alice = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("alice client");
    let bob = MlsRsClient::new(proto_suite, b"bob".to_vec()).expect("bob client");
    let carol = MlsRsClient::new(proto_suite, b"carol".to_vec()).expect("carol client");

    let mut alice_group = alice.create_group(config).expect("create group");
    let add_bob = alice_group.add_member(&bob).expect("add bob");
    let mut bob_group = bob
        .join_group(&add_bob.welcome_messages[0], add_bob.ratchet_tree.clone())
        .expect("bob join");

    let add_carol = alice_group.add_member(&carol).expect("add carol");
    let update = alice_group.self_update().expect("alice update");

    let out_of_order = bob_group
        .process_external_commit_explicit(ExternalCommit {
            epoch: 3,
            message: update.commit_message.clone(),
        })
        .expect_err("epoch gap must be rejected");
    assert!(matches!(out_of_order, MlsRsError::OutOfOrderCommit(_)));

    bob_group
        .process_external_commit_with_resync(
            ExternalCommit {
                epoch: 3,
                message: update.commit_message,
            },
            |from, to| {
                assert_eq!((from, to), (2, 3));
                Ok(vec![ExternalCommit {
                    epoch: 2,
                    message: add_carol.commit_message.clone(),
                }])
            },
        )
        .expect("resync path applies missing commit before target");

    assert_eq!(bob_group.epoch(), 3);
}

/// Validates context invariants for cfg_hash, proto_suite, and epoch checks.
#[test]
fn validates_context_invariants_for_cfg_proto_and_epoch() {
    let proto_suite = test_proto_suite();
    let mut group =
        Group::create(GroupConfig::new(proto_suite, 0, 1, vec![0x44; 32])).expect("create group");
    group.add_member("alice").expect("add member");

    let wrong_epoch = GroupMessage::new(
        group.epoch() + 1,
        group.cfg_hash().to_vec(),
        proto_suite,
        vec![],
    );
    assert!(matches!(
        group.validate_message(&wrong_epoch),
        Err(ValidationError::EpochMismatch { .. })
    ));

    let wrong_cfg = GroupMessage::new(group.epoch(), vec![0x99; 32], proto_suite, vec![]);
    assert!(matches!(
        group.validate_message(&wrong_cfg),
        Err(ValidationError::CfgHashMismatch)
    ));

    let wrong_proto = GroupMessage::new(
        group.epoch(),
        group.cfg_hash().to_vec(),
        ProtoSuite {
            major: 9,
            minor: 9,
            ciphersuite_id: 9,
        },
        vec![],
    );
    assert!(matches!(
        group.validate_message(&wrong_proto),
        Err(ValidationError::ProtoSuiteMismatch)
    ));
}

/// Rejects malformed external commit payloads before any MLS state transition.
#[test]
fn rejects_malformed_external_commit_payload() {
    let err = parse_external_commit(&[0xFF, 0x00, 0x11], 1).expect_err("must fail to parse");
    assert!(matches!(err, MlsRsError::MalformedExternalCommit(_)));
}

/// Rejects recovery attempts when fetched missing commits do not match expected epochs.
#[test]
fn rejects_incompatible_recovery_sequence() {
    let proto_suite = test_proto_suite();
    let mut group =
        Group::create(GroupConfig::new(proto_suite, 0, 1, vec![0x66; 32])).expect("create group");
    group.add_member("alice").expect("add member");

    let target = Commit::new(3);
    let missing = vec![Commit::new(3)];
    let err = group
        .apply_commit_with_recovery(target, missing)
        .expect_err("recovery sequence must match missing epochs exactly");
    assert!(matches!(err, MlsError::OutOfOrderCommit(_)));
}

/// Validates deterministic resync when commits are delivered in permuted order with add/update/remove.
#[test]
fn resync_handles_advanced_add_update_remove_permutations() {
    let proto_suite = test_proto_suite();
    let config = GroupConfig::new(proto_suite, 0, 1, vec![0x71; 32]);

    let alice = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("alice client");
    let bob = MlsRsClient::new(proto_suite, b"bob".to_vec()).expect("bob client");
    let carol = MlsRsClient::new(proto_suite, b"carol".to_vec()).expect("carol client");
    let dave = MlsRsClient::new(proto_suite, b"dave".to_vec()).expect("dave client");

    let mut alice_group = alice.create_group(config).expect("create group");
    let add_bob = alice_group.add_member(&bob).expect("add bob");
    let mut bob_group = bob
        .join_group(&add_bob.welcome_messages[0], add_bob.ratchet_tree.clone())
        .expect("bob join");
    let add_carol = alice_group.add_member(&carol).expect("add carol");
    bob_group
        .process_commit_message(add_carol.commit_message.clone())
        .expect("bob process add carol");
    let mut carol_group = carol
        .join_group(
            &add_carol.welcome_messages[0],
            add_carol.ratchet_tree.clone(),
        )
        .expect("carol join");

    let update = alice_group.self_update().expect("alice update");
    let remove_carol = alice_group
        .remove_member_by_identity(b"carol")
        .expect("remove carol");
    let add_dave = alice_group.add_member(&dave).expect("add dave");

    let result = bob_group
        .process_external_commit_with_resync(
            explicit_external(5, &add_dave.commit_message),
            |from, to| {
                assert_eq!((from, to), (3, 5));
                Ok(vec![
                    explicit_external(4, &remove_carol.commit_message),
                    explicit_external(3, &update.commit_message),
                ])
            },
        )
        .expect("resync applies update/remove before add");
    assert!(matches!(
        result.effect,
        mls_rs::group::CommitEffect::NewEpoch(_)
    ));
    assert_eq!(bob_group.epoch(), 5);
    assert_eq!(alice_group.epoch(), 5);
    assert_eq!(
        bob_group.member_identities(),
        alice_group.member_identities()
    );

    carol_group
        .process_external_commit_explicit(explicit_external(3, &update.commit_message))
        .expect("carol applies epoch 3 update");
    let stale = carol_group
        .process_external_commit_explicit(explicit_external(3, &update.commit_message))
        .expect_err("regressive epoch must be rejected");
    assert!(matches!(stale, MlsRsError::OutOfOrderCommit(_)));
}

/// Rejects partial resync recovery when only a subset of required commits can be fetched.
#[test]
fn rejects_partial_resync_recovery_with_missing_epochs() {
    let proto_suite = test_proto_suite();
    let config = GroupConfig::new(proto_suite, 0, 1, vec![0x72; 32]);

    let alice = MlsRsClient::new(proto_suite, b"alice".to_vec()).expect("alice client");
    let bob = MlsRsClient::new(proto_suite, b"bob".to_vec()).expect("bob client");
    let carol = MlsRsClient::new(proto_suite, b"carol".to_vec()).expect("carol client");
    let dave = MlsRsClient::new(proto_suite, b"dave".to_vec()).expect("dave client");

    let mut alice_group = alice.create_group(config).expect("create group");
    let add_bob = alice_group.add_member(&bob).expect("add bob");
    let mut bob_group = bob
        .join_group(&add_bob.welcome_messages[0], add_bob.ratchet_tree.clone())
        .expect("bob join");

    let add_carol = alice_group.add_member(&carol).expect("add carol");
    let _update = alice_group.self_update().expect("alice update");
    let add_dave = alice_group.add_member(&dave).expect("add dave");

    let err = bob_group
        .process_external_commit_with_resync(
            explicit_external(4, &add_dave.commit_message),
            |_, _| Ok(vec![explicit_external(2, &add_carol.commit_message)]),
        )
        .expect_err("must reject inconsistent partial recovery");
    assert!(matches!(err, MlsRsError::OutOfOrderCommit(_)));
    assert_eq!(bob_group.epoch(), 1);
}
