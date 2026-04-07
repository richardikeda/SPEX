# Transport and Ingestion Observability

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This document defines metrics, tracing, and network-health indicators for continuous `spex-transport` operations.

## Structured Metrics

Metrics are exposed via `P2pMetricsSnapshot`.

### Publish

- `publish_attempts`
- `publish_success`
- `publish_timeout`
- `publish_retries`
- `publish_latency_ms`
- `publish_success_rate_bps()`

### Recovery

- `recovery_attempts`
- `recovery_success`
- `recovery_timeout`
- `recovery_retries`
- `recovery_latency_ms`
- `recovery_timeout_rate_bps()`

### Bridge Fallback

- `fallback_attempts`
- `fallback_success`
- `fallback_failure`
- `fallback_frequency_bps()`

### Reassembly and Verification Errors

- `reassemble_failures`
- `verification_failures`

## Tracing and Correlation

- Correlation IDs are derived deterministically per operation.
- When full context is unavailable, minimal deterministic correlation fallback is used.
- Correlation metadata must never include raw payload bytes or key material.

Recommended trace fields:

- `operation`
- `correlation_id`
- `latency_ms`
- `attempt`
- `delay_ms`
- `items`

## Network Health Indicators

`network_health_indicators(thresholds)` exposes:

- `connected_peers`
- `known_peers`
- `banned_peers`
- `timeout_ratio_bps`
- `fallback_failure_ratio_bps`
- `status` (`healthy`, `degraded`, `critical`)

Default threshold guidance (`NetworkHealthThresholds::default`):

- `min_connected_peers = 2`
- `max_timeout_ratio_bps = 2500`
- `max_fallback_failure_ratio_bps = 3000`

## Operational Guidance

- **Healthy**: connectivity and failure ratios are within target.
- **Degraded**: threshold trend indicates elevated risk; investigate before escalation.
- **Critical**: low connectivity or severe ratio breach; execute incident response runbook.

## Runtime Hardening Signals

Track these additional counters for production hardening:

- reputation transitions (`probation`, `banned`)
- snapshot load status (`loaded`, `missing`, `quarantined`)
- restored state counters (peers/manifests/index)
- retry pressure indicators for churn windows

## Readiness Checklist

1. Timeout ratio remains below configured threshold.
2. Fallback failure ratio remains below configured threshold.
3. No sustained critical status windows.
4. Deterministic correlation IDs are stable for identical contexts.
5. Untrusted-input failures remain explicit and non-panicking.
