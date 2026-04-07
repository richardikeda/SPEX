# SPEX Deep Research Report

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

## Executive Summary

SPEX is a Rust-based protocol stack focused on secure permissioned exchange.
It combines:

- canonical CBOR/CTAP2 wire representation
- explicit grants and PoW anti-abuse controls
- MLS-based group encryption and epoch consistency checks
- hybrid delivery model (P2P plus HTTP bridge fallback)

The architecture aims for deterministic behavior and auditable failure modes under hostile network assumptions.

## Workspace Component Map

- `spex-core`: canonical types, crypto primitives, validation, invariant enforcement.
- `spex-mls`: MLS lifecycle, external commit handling, epoch gap/recovery controls.
- `spex-transport`: P2P runtime, manifests/chunking, ingestion validation, fallback orchestration.
- `spex-bridge`: HTTP bridge with SQLite persistence, abuse controls, bounded API behavior.
- `spex-client`: high-level local-state and messaging flow orchestration.
- `spex-cli`: operational entry point for identity, grants, threads, inbox, logs.

## Architectural Rationale

### Why canonical CBOR/CTAP2

Canonical CBOR reduces ambiguity and makes signatures/hashes deterministic across implementations.
This is critical for interoperable verification and replay-safe processing.

### Why MLS

MLS provides group-forward-secrecy and key schedule evolution.
SPEX adds explicit context validation (`cfg_hash`, epoch continuity) to avoid implicit acceptance paths.

### Why hybrid transport

P2P supports decentralized delivery and resilience.
HTTP bridge fallback supports environments with unstable connectivity or NAT constraints.
Both are treated as untrusted delivery channels.

## Threat and Risk Highlights

Primary risk categories:

- malformed input parsing and boundary handling
- replay/reorder/epoch desynchronization behavior
- abusive traffic and resource exhaustion
- supply-chain dependency risk
- local-state compromise and key-handling mistakes

Mitigation strategy:

- strict validation and explicit errors
- deterministic state machine transitions
- anti-abuse controls (PoW, rate limits, reputation)
- adversarial testing (negative suites, property tests, fuzzing)
- release gates for docs, quality, and critical test scope

## Operational Recommendations

- enforce TLS for all bridge and external API traffic
- protect local state with keychain/passphrase and strict permissions
- treat key changes as critical events requiring explicit user confirmation
- monitor timeout/fallback health indicators and ban/probation transitions
- keep advisory and license gates mandatory in CI/release

## Maturity Snapshot

Current maturity is appropriate for a security-focused open source protocol baseline, with active hardening areas:

- long-horizon P2P churn/soak behavior
- advanced MLS interop matrix expansion
- stateful fuzzing and differential parser validation

## Conclusion

SPEX is intentionally explicit, deterministic, and security-first.
Its design center is not convenience messaging, but auditable and permissioned communication for sensitive contexts.
