# Security Policy

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

## Supported Versions

| Version | Supported |
| --- | --- |
| main | Yes |
| v0.1.x | Yes |
| < v0.1.0 | No |

Only main and the latest stable release receive security fixes.

## Reporting a Vulnerability

Do not open public issues for vulnerabilities.

Use GitHub Security Advisory private reporting for this repository:

- https://github.com/richardikeda/SPEX/security/advisories/new

Please include:

- vulnerability description
- affected components
- impact
- reproduction steps
- proof-of-concept (if available)
- affected version/commit

Acknowledgment target: up to 72 hours.

## Disclosure Process

1. Report acknowledged.
2. Triage and severity classification.
3. Patch development and review.
4. Fix publication.
5. Coordinated disclosure after patch availability.

## Threat Model

SPEX assumes hostile conditions:

- untrusted networks
- compromised bridge/DHT paths
- interception and tampering attempts
- spam/DoS pressure
- malicious peers
- key compromise/rotation events

SPEX does not trust transport as a security boundary.

## Security Guarantees

SPEX targets:

- confidentiality
- integrity
- authenticity
- explicit permission model
- revocation/expiration controls
- anti-spam resistance
- forward secrecy in group messaging
- auditability without payload disclosure

## Non-Goals

SPEX does not guarantee:

- absolute anonymity in every scenario
- protection against full endpoint compromise
- immunity to advanced traffic correlation
- protection against social engineering
- replacement for secure operations

## Cryptography Baseline

SPEX uses established primitives, including:

- Ed25519
- SHA-256 / BLAKE3
- AEAD families (as implemented)
- MLS (RFC 9420)
- Argon2id PoW
- canonical CBOR (CTAP2 profile)

Primitive changes are treated as breaking security changes.

## Secure Defaults

- minimum PoW policy (memory >= 64 MiB, iterations >= 3)
- expired-grant rejection
- cfg_hash/epoch mismatch rejection
- key-change treated as critical event
- explicit trusted/untrusted input boundaries

## Local Storage Security

Implementations should:

- restrict file permissions
- encrypt local state/snapshots/exports
- detect rollback/corruption
- avoid sensitive plaintext in logs

Do not publish local state files.

## Transport Security

- Require HTTPS/TLS for bridge/external API paths.
- Treat DHT/gossip/bridge as untrusted delivery channels.
- Validate every message independently of transport.

## Supply Chain Security

Recommended controls:

- cargo audit
- cargo deny
- dependency review for critical crates
- CI quality gates (fmt/clippy/tests)

## Security Updates

Security fixes are prioritized and may trigger out-of-cycle releases.
All security-relevant fixes must be documented in CHANGELOG.md.
Users should monitor repository releases and changelog entries for remediation guidance.

## Contact and Coordination

- Private vulnerability disclosure: https://github.com/richardikeda/SPEX/security/advisories/new
- Public non-security questions: use GitHub issues with explicit technical context
- Security-sensitive reports must remain private until coordinated disclosure is complete

## Robustness Testing

SPEX adopts continuous robustness testing for parser and untrusted-input boundaries:

- fuzzing for critical CBOR and bridge/transport decoding paths
- property-based checks for determinism/idempotence
- explicit Result-based errors for untrusted data (no panic dependency)

Suggested manual commands:

```bash
cargo test -p spex-core
cargo test -p spex-bridge
cargo fuzz run parse_cbor_payload --manifest-path fuzz/Cargo.toml
```

Secure.
Permissioned.
Explicit.
