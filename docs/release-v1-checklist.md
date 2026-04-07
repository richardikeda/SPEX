# Release v1.0 Checklist (Go/No-Go)

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This checklist defines objective and reproducible criteria for the first definitive SPEX release.
All critical gates are explicit and can be executed locally or in CI.

## 1) Entry Criteria

A release candidate can start only when all conditions below are true:

- `VERSION.md` reflects the target release candidate patch.
- `CHANGELOG.md` contains an explicit v1 scope summary.
- No open protocol-format changes are present in the release branch.
- All backlog items listed as blockers in `TODO.md` are complete.

## 2) Mandatory Gates

Run all commands from repository root.

### Gate A — Critical Tests

```bash
cargo test -p spex-core -p spex-mls -p spex-transport --locked --all-features --verbose
```

Go if the command succeeds.
No-Go if the command fails.

#### Critical test classification criteria

A test is classified as **critical** only when it validates at least one of the rules below:

- protocol invariants that protect canonical encoding, signature validity, and deterministic state transitions;
- authentication/authorization paths where bypass would compromise security guarantees;
- transport/session controls that can break integrity, replay protection, or ordering guarantees.

To avoid ambiguity, the critical scope is explicit and package-scoped:

- `spex-core`
- `spex-mls`
- `spex-transport`

Any proposal to add/remove crates from this scope must include:

- a security rationale in the PR description;
- updates to this checklist and `.github/workflows/release-readiness.yml` in the same PR;
- successful execution of the updated critical suite.

### Gate B — Security/Robustness Regression

```bash
cargo test -p spex-core --locked --verbose
cargo test -p spex-mls --locked --verbose
cargo test -p spex-transport --locked --verbose
cargo test -p spex-bridge --locked --verbose
cargo test -p spex-client --locked --verbose
cargo test -p spex-cli --locked --verbose
```

Go if all protocol and integration suites pass.
No-Go if any crate fails.

### Gate C — Documentation and Quality

```bash
cargo fmt --all -- --check
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
./scripts/release_gate_docs.sh
```

Go if formatting, linting, and docs gate pass.
No-Go if any command fails.

## 3) Explicit Negative Gate (Block-on-Failure Behavior)

The release process must prove that critical failures block publication.

```bash
./scripts/release_gate_negative_test.sh
```

This script intentionally runs a failing command and asserts gate rejection behavior.
If this check does not detect failure, release is automatically No-Go.

## 4) Go/No-Go Decision Record

For each candidate, record:

- Candidate version and commit SHA.
- Gate outputs (A/B/C + negative gate).
- Reviewer responsible for approval.
- Final decision: `GO` or `NO-GO`.
- If `NO-GO`, blocking issue and mitigation owner.

## 5) Rollback Criteria

Rollback is mandatory if any of the following occurs after release:

- Authentication/authorization validation bypass.
- Canonical encoding/signature mismatch regressions.
- Determinism regressions in MLS epoch/cfg_hash checks.
- Operational instability with unrecoverable recovery-state corruption.

On rollback, publish an incident note and revert to the last known-good release.

## 6) Artifact Verification

Before publication, verify:

- Changelog scope matches implemented changes.
- All docs links referenced by `README.md` resolve.
- CI workflow status is green for release-required jobs.

## 7) Merge/Release Blocking Policy

`release-critical-tests` is a **mandatory status check** for `main`.
Merge and release are blocked when this check fails.

Branch protection/ruleset must enforce all conditions below:

- Require pull request before merging.
- Require status checks to pass before merging.
- Include `release-critical-tests` in required status checks.

Declarative branch protection policy is stored at:

- `.github/branch-protection/main.json`

Apply it with GitHub API (admin token required):

```bash
gh api \
  --method PUT \
  repos/:owner/:repo/branches/main/protection \
  --input .github/branch-protection/main.json
```
