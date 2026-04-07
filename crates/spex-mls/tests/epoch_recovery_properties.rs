use proptest::prelude::*;
use spex_core::types::ProtoSuite;
use spex_mls::{Commit, Group, GroupConfig};

/// Builds a deterministic protocol suite used across property tests.
fn test_proto_suite() -> ProtoSuite {
    ProtoSuite {
        major: 0,
        minor: 1,
        ciphersuite_id: 1,
    }
}

/// Builds a deterministic MLS group with one known member and epoch 1.
fn seeded_group() -> Group {
    let mut group = Group::create(GroupConfig::new(test_proto_suite(), 0, 1, vec![0x90; 32]))
        .expect("group creation must succeed");
    group
        .add_member("alice")
        .expect("seed member commit must succeed");
    group
}

// Property: recovery must be order-independent for fetched missing commits because implementation sorts by epoch.
proptest! {
    #[test]
    fn recovery_is_deterministic_under_missing_commit_permutations(reverse in any::<bool>()) {
        let mut group = seeded_group();
        let target = Commit::new(4);

        let mut missing = vec![Commit::new(2), Commit::new(3)];
        if reverse {
            missing.reverse();
        }

        group
            .apply_commit_with_recovery(target, missing)
            .expect("recovery with exact missing epochs must succeed");

        prop_assert_eq!(group.epoch(), 4);
    }
}

// Property: malformed recovery sequences must be rejected explicitly and keep local epoch unchanged.
proptest! {
    #[test]
    fn malformed_recovery_sequence_is_rejected(insert_unexpected_epoch in 5u64..10u64) {
        let mut group = seeded_group();
        let initial_epoch = group.epoch();

        let target = Commit::new(initial_epoch + 3);
        let missing = vec![
            Commit::new(initial_epoch + 1),
            Commit::new(insert_unexpected_epoch),
        ];

        let result = group.apply_commit_with_recovery(target, missing);
        prop_assert!(result.is_err());
        prop_assert_eq!(group.epoch(), initial_epoch);
    }
}

// Property: stale/current replay attempts must fail and keep the local epoch unchanged.
proptest! {
    #[test]
    fn stale_or_current_replay_does_not_advance_epoch(offset in 0u64..4u64) {
        let mut group = seeded_group();
        let initial_epoch = group.epoch();
        let replay_epoch = initial_epoch.saturating_sub(offset);

        let result = group.apply_commit(Commit::new(replay_epoch));
        prop_assert!(result.is_err());
        prop_assert_eq!(group.epoch(), initial_epoch);
    }
}
