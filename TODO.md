# SPEX TODO List (v1.0.3)

## Scope

This TODO defines the remaining items that must be completed by the maintainer before
a public release is declared. All three original v1.0 blockers have been resolved in code;
the items below are either maintainer gate actions (cannot be automated) or post-1.0 backlog.

---

## [RELEASE GATE] Create and Push Git Version Tags

Why this blocks public release:
- No version tags exist in the repository (`git tag --list` returns empty).
- Release traceability requires that declared versions map to specific commits.
- Users and tools that depend on tagged releases cannot pin to a version without tags.

Required action (maintainer):
```bash
git tag v1.0.0 <sha-for-1.0.0>
git tag v1.0.1 <sha-for-1.0.1>
git tag v1.0.2 <sha-for-1.0.2>
git tag v1.0.3 HEAD
git push origin v1.0.0 v1.0.1 v1.0.2 v1.0.3
```

Identify the correct commit SHAs using `git log --oneline` and matching the commit dates
in `CHANGELOG.md` entries.

Acceptance criteria:
- `git tag --list` returns at minimum `v1.0.3`.
- `CHANGELOG.md` "Published Versions" section matches the tags present in the repository.

---

## [RELEASE GATE] TLS Validation Checklist — Generate Release Evidence

Why this blocks public release:
- `docs/security.md` requires that the TLS validation checklist is run before any production
  deployment and that release evidence includes successful checklist output.
- The checklist requires a live deployment to verify (certificate, protocol, redirect,
  plain-HTTP rejection).

Required action (maintainer):
- Deploy the bridge behind a TLS-terminating reverse proxy (see `docs/bridge-tls-deployment.md`).
- Run each checklist step in that document and capture the output.
- Include the output in the release notes or CI artifacts as release evidence.

Acceptance criteria:
- All six checklist steps in `docs/bridge-tls-deployment.md` produce expected output.
- Evidence is attached to the v1.0.3 release.

---

## [POST-1.0 BACKLOG] Migrate serde_cbor to ciborium

Why it is tracked:
- `serde_cbor` (RUSTSEC-2021-0127) is unmaintained. A formal risk-acceptance record with
  expiration date 2026-10-01 is in `deny.toml`.
- Migration must be completed or the exception renewed before 2026-10-01.

Migration scope:
- `spex-core/src/cbor.rs` — replace `serde_cbor` with `ciborium` for CTAP2 encoding/decoding.
- `spex-core/src/hash.rs` — update hash helpers that depend on CBOR value types.
- `crates/spex-client/` — state serialization paths using `serde_cbor`.
- Verify CTAP2 canonical encoding parity with existing test vectors before merging.

Acceptance criteria:
- `serde_cbor` removed from all `Cargo.toml` files.
- `RUSTSEC-2021-0127` ignore removed from `deny.toml`.
- All existing CBOR tests pass with `ciborium` backend.
- `cargo deny check` returns clean with no advisory exceptions.

---

## Post-1.0 Backlog (Not Blocking v1.0 Closure)

- Longer-duration transport churn/soak campaigns and expanded anti-eclipse thresholds.
- Advanced MLS cross-implementation interop matrix expansion.
- Stateful and differential fuzz campaign expansion beyond release smoke baseline.
- Broader CI matrix expansion (for example, Windows/macOS full test execution).
- Observability exporter standardization and dashboard packaging.
