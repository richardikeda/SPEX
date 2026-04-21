# Security

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This document summarizes mandatory practices and operational recommendations for SPEX integrations.

## Canonical CBOR (CTAP2)

SPEX requires canonical CBOR (CTAP2) for deterministic serialization and signature verification.
This prevents byte-level ambiguity across implementations and reduces canonicalization attack surface.

## Card Validation and Key Change Handling

Cards must pass strict validation:

- Verify signature when present.
- Reject invalid fields, non-canonical encoding, or inconsistent timestamps.
- Treat key changes as critical events requiring explicit user confirmation or revocation workflow.

Unexpected key changes may indicate compromise or substitution attack.

## Request/Grant and Permission Controls

- Always validate RequestToken before issuing grants.
- Enforce minimum PoW when requires_puzzle is active (memory >= 64 MiB, iterations >= 3).
- Restrict roles/flags under local least-privilege policy.

Anti-spam economic friction model and full bridge ingress validation pipeline: see [docs/diagrams.md — Anti-Spam & Abuse Controls](diagrams.md#7-anti-spam--abuse-controls) and [docs/diagrams.md — Bridge Validation Pipeline](diagrams.md#4-bridge-validation-pipeline).


## P2P Persistence and Anti-Eclipse Controls

To reduce state loss and eclipse risk:

- Persist peer/bootstrap state with deterministic atomic snapshot writes.
- Treat corrupted snapshots as untrusted input and recover with explicit warnings.
- Use peer scoring penalties for invalid payloads, recurring timeouts, and inconsistent responses.
- Apply temporary bans for peers crossing critical abuse thresholds.

## Mandatory TLS

Use TLS/HTTPS for all bridge and external API traffic.
TLS protects metadata and transport integrity but does not replace protocol signature/context checks.

The mandatory deployment model is **reverse-proxy TLS termination**:

- The `spex-bridge` binary listens on a local plain-HTTP port (default `127.0.0.1:3000`).
- A TLS-terminating reverse proxy (nginx, Caddy, HAProxy) handles all external HTTPS connections.
- The bridge socket must **never** be directly exposed to the public internet over plain HTTP.
- TLS 1.2 minimum; TLS 1.0 and 1.1 must be disabled at the proxy layer.
- Certificates must be valid, unexpired, and trusted by a standard CA store.

Full TLS deployment guide, configuration examples, and the operator validation checklist:
**[docs/bridge-tls-deployment.md](bridge-tls-deployment.md)**

Operators must run the validation checklist before any production deployment.
Release evidence must include successful checklist output.

## Grant Expiration

Grants should carry expires_at whenever possible:

- reject expired tokens immediately
- prefer short-lived temporary permissions
- reissue/revoke grants on key-change or trust-loss events

Non-expiring grants should be exceptional and periodically reviewed.

## Protecting ~/.spex/state.json

Local state contains keys, contacts, and thread metadata. Recommended controls:

- strict file/dir permissions
- keychain-backed encryption or SPEX_STATE_PASSPHRASE fallback
- secure backup handling and access isolation

Avoid storing state in shared or exposed locations.

## Revocation and Recovery via Checkpoint Log

The append-only checkpoint log supports verifiable revocation and recovery:

- publish key checkpoints and revocation records
- distribute logs via redundant channels and verify prefix consistency
- use registered recovery keys for compromised identities

Clients should verify Merkle roots and reject inconsistent log histories.


## Adversarial Robustness (Fuzz + Property Tests)

Untrusted input boundaries must be continuously tested:

- run fuzz targets for critical parsing/decoding boundaries
- enforce property tests for deterministic/idempotent validation behavior
- include malformed/truncated/type-confusion negative tests
- require explicit errors instead of panic paths for untrusted data

Recommended smoke commands before release:

```bash
cargo test -p spex-transport p2p_ingest_property
cargo test -p spex-bridge adversarial_parsing
cargo test -p spex-core --test ctap2_cbor_vectors -- --nocapture
cargo test -p spex-transport --test p2p_manifest_recovery -- --nocapture
for target_file in fuzz/fuzz_targets/*.rs; do
  target_name="$(basename "${target_file}" .rs)"
  cargo +nightly fuzz run "${target_name}" --fuzz-dir fuzz -- -max_total_time=30 -seed=1
done
```

Additional robustness coverage in this phase:

- transport_manifest_gossip_parse fuzz target for untrusted gossip boundary parsing
- reinforced adversarial tests for bridge, transport, and core vectors

Fuzz smoke policy for release readiness:

- CI installs cargo-fuzz in robustness job
- all fuzz targets run with deterministic time/seed settings
- any crash/panic fails the job


## Advisory Response (cargo-audit / cargo-deny)

When advisories are detected, treat as security incidents:

1. Immediate triage
   - identify advisory id/package/version/severity
   - determine real execution-path exposure

2. Containment
   - block release while unresolved on release branch
   - disable optional features when needed

3. Remediation
   - prioritize upstream patched versions
   - document temporary mitigation and residual risk if no patch exists
   - keep deny.toml ignores temporary and justified

4. Validation
   - rerun cargo audit and cargo deny check locally and in CI
   - rerun relevant regression suites

5. Traceability and communication
   - document root cause, impact, mitigation, and fixing commit hash
   - track follow-up to remove temporary workarounds

Exit criterion: release only with green supply-chain pipeline and no unresolved advisories without formal approved exception.
