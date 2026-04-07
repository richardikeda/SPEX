# SPEX Advanced Security and Threat Testing Playbook

## Objective

Define an implementation-ready test strategy for advanced security validation across SPEX components:
- MLS state and epoch safety
- P2P transport abuse-resistance
- Bridge API hardening and authorization boundaries
- Stateful protocol fuzzing
- Memory, side-channel, and physical threat coverage

This playbook is designed to be executed incrementally while preserving core protocol invariants:
- canonical CBOR behavior
- deterministic hashing and cfg_hash invariants
- signature authenticity and verification strictness
- epoch consistency and anti-replay guarantees

## Scope and Security Model

Components in scope:
- `spex-core`
- `crates/spex-mls`
- `crates/spex-transport`
- `crates/spex-bridge`
- `crates/spex-client`
- `fuzz/`

Out of scope:
- cryptographic primitive redesign
- wire format changes without explicit protocol approval

Security assumptions to continuously validate:
- All untrusted inputs must fail with explicit errors.
- No critical security path may rely on panic behavior.
- Permission and grant checks must remain explicit and deny-by-default.
- Failure behavior must be deterministic and auditable.

## Threat Matrix by Component

### 1) MLS (`crates/spex-mls`)

Threats:
- Epoch desynchronization and stale commit acceptance
- Forged/invalid external commit acceptance
- Group membership confusion (removed member re-entry without authorization)
- State rollback/replay of old group context

High-value tests:
- Replay old commits against newer epoch and assert explicit rejection.
- Inject malformed proposal/commit payloads and assert parse + validate failures.
- Attempt commits from revoked identities and assert membership denial.
- Property tests for monotonic epoch progression and deterministic state transitions.

### 2) P2P Transport (`crates/spex-transport`)

Threats:
- Chunk reordering, duplication, truncation, and mixed-session reassembly
- PoW bypass attempts and grant misuse
- Telemetry abuse and input amplification
- Resource exhaustion via high-rate malformed payload streams

High-value tests:
- Adversarial chunk streams (out-of-order, duplicated, missing final chunk).
- Invalid/missing PoW metadata and downgraded difficulty attempts.
- Forged grant payloads across identity boundaries.
- Stress tests with malformed envelopes under bounded memory assertions.

### 3) Bridge (`crates/spex-bridge`)

Threats:
- Authorization bypass in API handlers
- Tenant/session mix-up under concurrent access
- Export/import abuse and schema confusion
- Log/event tampering visibility gaps

High-value tests:
- Negative auth matrix per endpoint (missing, malformed, cross-identity grants).
- Concurrency abuse tests for session isolation and deterministic conflict handling.
- Structured malformed request corpus for parser and validator boundaries.
- Auditability checks: expected security events must be emitted for denied actions.

### 4) Core Serialization and Validation (`spex-core`)

Threats:
- Non-canonical CBOR acceptance leading to signature/hash ambiguity
- Validation bypass via edge-case encoding
- Type confusion on decode boundaries

High-value tests:
- Canonical/non-canonical differential test vectors.
- Round-trip determinism properties for signed structures.
- Corruption tests for each critical field and explicit error mapping assertions.

## Stateful Fuzzing Strategy

## Goals

- Discover invalid state transitions and panic paths in protocol state machines.
- Validate that malformed sequences do not create silent acceptance paths.

## Targets (priority)

1. `mls_parse_external_commit`
2. `parse_cbor_payload`
3. `inbox_store_request_from_bytes`
4. `p2p_grant_payload_validation`
5. `p2p_puzzle_payload_validation`

## Enhancements

- Add sequence-aware fuzz harnesses that model operation ordering:
  - create -> grant -> publish -> revoke -> recover
  - join -> commit -> remove -> replay old commit
- Add corpus seeds from real protocol traces and known malformed edge cases.
- Add sanitizer matrix in CI for fuzz regression checks (ASan/UBSan where feasible).
- Define crash triage rules:
  - security bug if it can bypass validation/authz or corrupt state invariants
  - reliability bug if panic/resource exhaustion only

## Memory, Side-Channel, and Physical Threat Testing

### Memory Safety and Resource Controls

- Run tests with constrained memory settings and large malformed inputs.
- Add explicit upper-bound assertions for chunk assembly, inbox parsing, and request buffering.
- Add timeout and cancellation path tests for long-running operations.

### Side-Channel Awareness

- Timing variance smoke checks around critical verify paths (non-constant-time anomalies as alerts).
- Ensure error messages do not leak sensitive state (key material, internal identifiers).

### Physical/Operational Threat Simulation

- Simulate clock skew and restart/recovery across epochs.
- Simulate partial disk corruption of persisted state and verify fail-closed behavior.
- Simulate key material unavailability and ensure explicit operational errors.

## Test Design Requirements

Each new security test should include:
- clear threat statement
- invariant(s) under test
- expected deny/fail behavior
- explicit error assertion (not only boolean failure)
- regression tag or naming for traceability

Naming guidance:
- `security_<component>_<threat>_<expected_behavior>`

Examples:
- `security_mls_replay_old_epoch_rejected`
- `security_transport_invalid_pow_denied`
- `security_bridge_cross_tenant_grant_denied`

## Implementation Plan (Phased)

### Phase A: Fast Security Regressions (Immediate)

- Add focused negative tests in existing test modules for:
  - MLS replay and stale epoch rejection
  - transport invalid PoW / grant misuse rejection
  - bridge authz-deny matrix for critical endpoints
- Add minimal deterministic assertions for error types/messages.

Exit criteria:
- All new tests green in default CI path.
- No panic paths on malformed security-critical inputs.

### Phase B: Property and Differential Expansion

- Add property-based tests for epoch monotonicity and deterministic state.
- Add canonical CBOR differential vectors in `spex-core`.
- Expand malformed corpus for parser boundaries.

Exit criteria:
- Property tests stable and non-flaky.
- Differential vectors fully deterministic.

### Phase C: Stateful Fuzzing and Hardening

- Introduce sequence-aware fuzz targets for end-to-end state machines.
- Add fuzz corpus management and triage workflow.
- Run periodic fuzz campaigns and retain minimized crashing inputs.

Exit criteria:
- No open high-severity fuzz findings.
- Reproducible fuzz regression job available.

### Phase D: Operational/Physical Resilience

- Add recovery-path tests for restart, partial corruption, and key unavailability.
- Add bounded-resource stress tests for service interfaces.

Exit criteria:
- Fail-closed behavior validated under defined operational fault scenarios.

## CI and Release Gate Integration

Recommended security gate stack:
- `cargo test --workspace`
- focused negative/security suites per crate
- `cargo deny check`
- selected fuzz regression smoke runs with fixed time budget

Release must be blocked when:
- any critical security test fails
- supply-chain advisories are unresolved or undocumented
- fuzz regressions indicate crash or invariant bypass in critical parsers

## Traceability and Reporting

For each implemented test group, record:
- component
- threat category
- invariant validated
- failure mode
- linked issue/task

Suggested report file for periodic updates:
- `docs/security-test-progress.md`

## Immediate Backlog (Actionable)

1. Add MLS replay/stale epoch deny tests in `crates/spex-mls/tests/`.
2. Add invalid PoW and cross-identity grant abuse tests in `crates/spex-transport/tests/`.
3. Add bridge authz negative matrix tests in `crates/spex-bridge/tests/`.
4. Add canonical CBOR differential vectors in `spex-core/tests/`.
5. Add at least one sequence-aware fuzz target under `fuzz/fuzz_targets/`.

## Done Definition for This Playbook

This playbook is considered implemented when:
- all Phase A items are merged with green CI,
- at least 2 Phase B properties are active,
- at least 1 sequence-aware fuzz target runs in CI smoke mode,
- release checklist references these security gates explicitly.
