# Release v1.0.0 Pending Findings Resolution Report

Date: 2026-04-07
Scope: closure of previously identified blockers for v1.0.0 release decision.

## 1) Critical Finding: cargo deny bans/licenses failures

Original issue:

- advisories passed, but bans and licenses failed.
- Main causes:
  - internal crates missing explicit license fields
  - internal path dependencies missing explicit versions
  - transitive license CDLA-Permissive-2.0 not allowlisted

Resolution:

- Added explicit license fields to internal crates.
- Added explicit versions for internal path dependencies.
- Updated deny policy allowlist to include CDLA-Permissive-2.0.

Validation:

- `cargo deny check` passed (`advisories ok, bans ok, licenses ok, sources ok`).

## 2) Critical Finding: missing formal go/no-go decision record

Resolution:

- Added formal record: `docs/release-v1-go-no-go-record.md`.

Validation:

- Candidate version, SHA, gate outputs, and decision are explicitly documented.

## 3) Critical Finding: version/changelog mismatch with v1.0.0 release state

Resolution:

- Updated `VERSION.md` to release value.
- Updated `CHANGELOG.md` with explicit v1.0.0 release section and published-version alignment.

Validation:

- Release metadata synchronized.

## 4) Important Finding: outdated TODO status

Resolution:

- TODO is now focused on active pending backlog only.
- Completed historical content was moved into concise changelog entries.

Validation:

- TODO reflects actionable backlog rather than closed history.

## 5) Important Finding: test documentation drift

Resolution:

- Updated test documentation to reflect active suites and known ignored coverage.

Validation:

- Test docs and execution reality are now aligned.

## 6) Additional Regression Stability Fix

Issue found during revalidation:

- A CLI negative-path assertion was too strict for expected 4xx behavior.

Resolution:

- Assertion updated to require client-error class semantics without overfitting a single status code.

Validation:

- Targeted negative regression test passed.

## 7) Executed Evidence

The following commands were executed successfully during closure:

- `cargo deny check`
- `cargo fmt --all -- --check`
- `cargo test --workspace --locked -q`

## 8) Recommended Direction for v2.0

1. Continue advanced security campaigns (stateful fuzzing and differential validation).
2. Expand interop/version negotiation policy and compatibility matrix.
3. Keep supply-chain governance strict with temporary exceptions ownership/expiry.
4. Formalize SLO/error-budget operations for runtime reliability.
5. Stabilize SDK/API compatibility contracts and automate release evidence packaging.
