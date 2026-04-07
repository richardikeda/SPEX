# SPEX TODO List

## Scope

This TODO tracks active pending work only.
Completed items were moved into concise changelog entries.

## [TASK 1] Transport Runtime Production Hardening

Objective:
- Improve long-running P2P resilience under churn and partial network failure.
- Preserve deterministic behavior and explicit error paths.

Pending work:
- Longer-duration churn and soak testing.
- Expanded anti-eclipse telemetry and alert thresholds.
- Additional deterministic restart/recovery scenarios.

Acceptance criteria:
- No panic paths under malformed/untrusted input.
- Stable degraded/critical classification across repeated runs.
- Clear documented operator actions for critical transport alerts.

## [TASK 2] Advanced MLS Interop and Failure Matrix Expansion

Objective:
- Expand MLS safety coverage for replay, reorder, epoch-gap, and recovery mismatch scenarios.

Pending work:
- Mixed-topology recovery edge scenarios.
- Cross-implementation MLS compatibility matrix definition.

Acceptance criteria:
- Deterministic rejection semantics for invalid epoch/order flows.
- Explicit compatibility and fallback documentation.

## [TASK 3] Adversarial Robustness Campaign (Fuzz + Property)

Objective:
- Expand parser and boundary robustness for core, bridge, and transport.

Pending work:
- Stateful fuzz sequences across lifecycle transitions.
- Differential parser checks for malformed edge classes.

Acceptance criteria:
- No crash/panic in critical parsing boundaries during smoke campaigns.
- Explicit error returns for malformed/truncated/invalid type inputs.

## Additional Product and Integration Backlog

- Stabilize SDK/API compatibility policy and version matrix.
- Expand integration examples for external API consumers.
- Automate release-evidence packaging for go/no-go records.
