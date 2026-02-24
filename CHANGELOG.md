# Changelog

All notable changes to this project are documented in this file.

This project follows **Semantic Versioning**:  
https://semver.org/

---

## [Unreleased]

### Scope
- Work in progress after `0.1.65`.
- Current repository version is tracked in `VERSION.md`.

### Notes
- Changes since `0.1.65` are not part of a published release section yet.

### Documentation
- Reorganizado `TODO.md` para conter apenas backlog acionável, removendo blocos de funcionalidades já entregues.
- Itens concluídos sobre inbox write/scan na bridge, integração MLS existente e runtime P2P operacional foram consolidados no histórico de documentação e referenciados no roadmap do `README.md`.
- Alinhada a consistência de status entre `README.md`, `TODO.md` e `docs/bridge-api.md`.

---

## Published Versions

- `0.1.65` (latest published)
- `0.1.64` ... `0.1.0`

This list is intentionally aligned with `VERSION.md`: repository state is ahead of latest published release.

---

## [0.1.51 - 0.1.65] - Security Hardening and Test Expansion

### Security / Behavior Summary
- Hardened handling of secrets and runtime configuration to reduce accidental credential exposure.
- Tightened validation and failure-path behavior in bridge/client flows.
- Increased deterministic validation coverage (PoW/signature/hash paths) and regression protection.

### Notable Workstreams
- Security fixes around sensitive value handling and safer defaults.
- Expanded protocol and bridge tests, including negative/failure-oriented scenarios.
- CI and formatting stabilization to reduce drift between local and automated checks.

### Commit / Tag Cross-References
- Representative commits: `de23072`, `2e02633`, `654fc38`, `61c86f0`, `5f02c1f`, `a8cb92f`.
- Tag references: no version tags found in repository (`git tag --list` returned empty).

---

## [0.1.21 - 0.1.50] - Bridge Reliability, Performance, and Validation Improvements

### Security / Behavior Summary
- Improved explicit validation boundaries in bridge ingestion and message publication flows.
- Strengthened reliability under chunked/inbox transport scenarios.
- Reduced non-deterministic behavior risks via cleanup and helper deduplication.

### Notable Workstreams
- Bridge inbox write/ingest integration enhancements.
- Performance optimizations in validation/reassembly hot paths.
- Additional interoperability and integration scenarios.

### Commit / Tag Cross-References
- Representative commits: `a1f7534`, `81468ad`, `8ccb27b`, `2a0a7de`, `1838769`.
- Tag references: no version tags found in repository (`git tag --list` returned empty).

---

## [0.1.1 - 0.1.20] - Post-Release Stabilization of Initial SPEX Protocol

### Security / Behavior Summary
- Consolidated baseline protocol invariants after initial release (permissions, signatures, canonical encoding).
- Improved error handling and developer/operator visibility without weakening validation.
- Progressive test hardening for protocol edge cases and compatibility vectors.

### Notable Workstreams
- Early bug fixes and compatibility corrections.
- Validation and wire-format conformance adjustments.
- Initial expansion of automated checks and test vectors.

### Commit / Tag Cross-References
- Historical patches in this range are aggregated due limited per-patch metadata in this changelog.
- Use repository commit history for detailed forensic traceability (`git log --oneline`).

---

## [0.1.0] - Initial Public Release

### Added
- Core SPEX protocol implementation.
- Ed25519 identity and signature verification.
- Grant tokens with expiration and capability flags.
- PoW validation with minimum security parameters.
- Canonical CBOR serialization (CTAP2).
- MLS integration for encrypted threads.
- HTTP bridge API with explicit validation.
- Chunked transport and inbox scanning.
- CLI tooling for identity, cards, grants, and messaging.
- Initial documentation set.

### Security
- End-to-end encryption enforced.
- Context binding via `cfg_hash` and `epoch`.
- Transport treated as untrusted by design.
- Key changes treated as critical events.

---

## Versioning Notes

- Patch releases (`x.y.z`) include bug fixes and security patches.
- Minor releases (`x.y`) may add features without breaking compatibility.
- Major releases (`x`) may introduce breaking protocol changes.

Breaking changes are always documented explicitly.

---

**Secure.  
Permissioned.  
Explicit.**

- Added fuzzing and property-based robustness tests for CBOR/core decoding and bridge payload parsing.
