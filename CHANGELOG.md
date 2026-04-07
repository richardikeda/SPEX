# Changelog

All notable changes to this project are documented in this file.

This project follows **Semantic Versioning**:  
https://semver.org/

---

## [Unreleased]

### Scope
- Post-v1 maintenance and v2 planning.

---

## [1.0.3] - 2026-04-07

### License Alignment
- Aligned all crate `Cargo.toml` files to `MPL-2.0`, resolving the inconsistency between the
  `LICENSE.MD` / `README.md` (MPL-2.0) and the previous `Apache-2.0 OR MIT` manifest fields.
- `cargo deny check` passes with the corrected license declarations.

### Version Coherence
- Added `CHANGELOG.md` entry for the previously undocumented `1.0.2` version.
- Incremented `VERSION.md` from `1.0.2` to `1.0.3` per AGENTS.md versioning rules.
- Documented the git tag creation requirement in the release runbook: maintainers must
  create and push version tags (`git tag v1.0.0`, `v1.0.1`, `v1.0.2`, `v1.0.3`) after this
  release is merged to establish full provenance traceability.

### Security Exception Formalization
- Replaced the informal `deny.toml` comment for `RUSTSEC-2021-0127` (`serde_cbor`) with a
  structured risk-acceptance record: expiration date, owner, mitigation strategy, and removal plan.
- Documented the migration path to `ciborium` as the tracked follow-up item.

### TLS Production Model
- Defined the mandatory reverse-proxy TLS deployment model for the bridge server.
- Added `docs/bridge-tls-deployment.md` with step-by-step TLS configuration guidance,
  validation checklist, and reverse-proxy examples.
- Updated the previously `#[ignore]` TLS validation checklist in `spex-bridge/tests/integration.rs`
  to an executable test that verifies the bridge can serve requests over a TLS-terminated path.
- Updated `docs/security.md` with concrete TLS production deployment requirements.

---

## [1.0.2] - 2026-04-07

### Documentation
- Refactored core governance and protocol documentation for clarity and consistency.
- Standardized English across all docs; removed stale or duplicate content.
- No functional code changes in this version.

### Notes
- `VERSION.md` was incremented to `1.0.2` during documentation cleanup.
- No git tag was created at the time; see git tag creation requirement in `1.0.3` entry above.

---

## [1.0.1] - 2026-04-07

### Documentation and Governance
- Migrated Portuguese documentation to English across core repository docs.
- Added institutional metadata to public docs: project creation year (2026), author (Richard Ikeda), and open-source positioning.
- Clarified that SPEX was initially personal-use and is published open source for adoption and code verification.

### Open Source Readiness
- Standardized governance and security messaging for public contributors and reviewers.
- Consolidated references to release checklist, runbook, branch protection policy, and CI gates.
- Added a single PT-BR guide for users covering project purpose, architecture rationale, usage, and API integration paths.

### TODO and Backlog Hygiene
- Removed completed historical items from TODO and kept only active pending work.
- Moved completed task history into changelog-level concise records.

---

## [1.0.0] - 2026-04-07

### Release Summary
- First stable SPEX protocol release with v1 closure gates satisfied.
- Release checklist and operational runbook finalized for reproducible go/no-go decisions.

### Security / Supply Chain
- Resolved cargo-deny blockers across advisories, bans, licenses, and source policy.
- Upgraded vulnerable transitive dependencies in lockfiles.
- Added explicit temporary exception for `RUSTSEC-2021-0127` (`serde_cbor`) with documented mitigation policy.
- Removed wildcard internal dependency declarations by pinning local crate versions.

### Testing and Quality
- Full workspace regression revalidated (`cargo test --workspace --locked -q`).
- Formatting gate validated (`cargo fmt --all -- --check`).
- Supply-chain gate validated (`cargo deny check`).
- Stabilized CLI negative-path assertion for invalid PoW to enforce explicit 4xx rejection semantics.

### Documentation
- Synchronized release status in `TODO.md` and `TESTS.md` with actual validated gate state.
- Added formal go/no-go decision record for v1.0.0.
- Added resolution report for all pending blockers plus v2 direction.

---

## Published Versions

- `1.0.3` (current — awaiting git tag and release publication)
- `1.0.2` (no git tag created — documentation-only update)
- `1.0.1` (no git tag created)
- `1.0.0` (no git tag created)
- `0.1.65` ... `0.1.0`

**Git tag status**: No version tags have been created in the repository (`git tag --list` returns
empty). Maintainers must create and push tags for all declared versions to establish release
provenance. At minimum, create `v1.0.3` before publication. Tags for historical versions are
optional but recommended.

This list is aligned with `VERSION.md`.

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
