use spex_core::types::ProtoSuite;
use spex_mls::{Commit, Group, GroupConfig, MlsError};

/// Builds a deterministic protocol suite used by advanced negative MLS tests.
fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

/// Creates a deterministic group at epoch 1 with one known member.
fn seeded_group() -> Group {
    let mut group =
        Group::create(GroupConfig::new(test_proto_suite(), 0, 1, vec![0x81; 32])).expect("group");
    group.add_member("alice").expect("add alice");
    group
}

/// Rejects re-ordered delivery where epoch N+2 arrives before epoch N+1.
#[test]
fn rejects_reordered_commit_delivery_without_recovery() {
    let mut group = seeded_group();
    assert_eq!(group.epoch(), 1);

    let err = group
        .apply_commit(Commit::new(3))
        .expect_err("epoch 3 without epoch 2 must be rejected");
    assert!(matches!(err, MlsError::OutOfOrderCommit(_)));
    assert_eq!(group.epoch(), 1);
}

/// Rejects replay attempts when an already applied commit is received again.
#[test]
fn rejects_replayed_commit_deterministically() {
    let mut group = seeded_group();

    let commit_epoch_2 = Commit::new(2);
    group
        .apply_commit(commit_epoch_2.clone())
        .expect("first delivery of epoch 2 commit must succeed");

    let err = group
        .apply_commit(commit_epoch_2)
        .expect_err("replayed epoch 2 commit must be rejected as stale");
    assert!(matches!(err, MlsError::OutOfOrderCommit(_)));
    assert_eq!(group.epoch(), 2);
}

/// Rejects explicitly out-of-order commits with stale epochs.
#[test]
fn rejects_stale_epoch_commit_out_of_order() {
    let mut group = seeded_group();
    assert_eq!(group.epoch(), 1);

    let err = group
        .apply_commit(Commit::new(1))
        .expect_err("commit at current epoch must be rejected");
    assert!(matches!(err, MlsError::OutOfOrderCommit(_)));
    assert_eq!(group.epoch(), 1);
}

/// Rejects partial recovery when required intermediate epochs are missing.
#[test]
fn rejects_partial_recovery_for_missing_epoch_chain() {
    let mut group = seeded_group();
    let initial_epoch = group.epoch();

    let target = Commit::new(4);
    let missing = vec![Commit::new(2)];

    let err = group
        .apply_commit_with_recovery(target, missing)
        .expect_err("partial recovery must be rejected");
    assert!(matches!(err, MlsError::OutOfOrderCommit(_)));
    assert_eq!(group.epoch(), initial_epoch);
}
