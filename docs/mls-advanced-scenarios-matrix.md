# MLS Advanced Scenarios Matrix

This matrix tracks advanced MLS reliability/security scenarios required for v1 readiness,
with explicit mapping to automated tests in `crates/spex-mls/tests/*`.

## Matrix

| Scenario | Risk / Failure Mode | Primary Test Mapping | Deterministic Negative Coverage |
|---|---|---|---|
| Reorder (`N+1` delayed, `N+2` arrives first) | Group state divergence or accepting commits in invalid order. | `crates/spex-mls/tests/planned_concurrent_updates.rs::receives_n_plus_one_without_n_then_recovers` (recovery path) | `crates/spex-mls/tests/mls_advanced_negative.rs::rejects_reordered_commit_delivery_without_recovery` |
| Replay (same commit delivered twice) | Duplicate state transitions and replay window abuse. | `crates/spex-mls/tests/planned_concurrent_updates.rs::resync_handles_advanced_add_update_remove_permutations` (stale replay rejection via explicit external commit) | `crates/spex-mls/tests/mls_advanced_negative.rs::rejects_replayed_commit_deterministically` |
| Out-of-order epoch (stale/current epoch received) | Accepting invalid epochs that violate monotonic progression. | `crates/spex-mls/tests/planned_concurrent_updates.rs::concurrent_add_and_remove_same_interval_is_rejected` | `crates/spex-mls/tests/mls_advanced_negative.rs::rejects_stale_epoch_commit_out_of_order` |
| Partial recovery (subset of missing commits) | Unsafe recovery with epoch gaps or incomplete state reconstruction. | `crates/spex-mls/tests/planned_concurrent_updates.rs::rejects_partial_resync_recovery_with_missing_epochs` | `crates/spex-mls/tests/mls_advanced_negative.rs::rejects_partial_recovery_for_missing_epoch_chain` |

## Notes

- All negative tests above are deterministic and assert both explicit error behavior and epoch invariants (no unintended state advance).
- This matrix is referenced by the v1 readiness section in `README.md`.
