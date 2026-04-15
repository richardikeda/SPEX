# Release Operations Runbook (v1.0)

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

This runbook consolidates operational actions for go-live, incident triage, and rollback.

## 1) Pre-Go-Live

1. Confirm all mandatory release gates passed (`docs/release-v1-checklist.md`).
2. Confirm monitoring dashboards include publish/recovery/fallback metrics.
3. Confirm on-call and incident channels are staffed.
4. Confirm backup/export path for append-only logs is available.

## 2) Go-Live Sequence

1. Prepare release branch and open PR to `main`.
2. Ensure all commits in the release branch are signed and verifiable.
3. Require CI green and CodeQL with no new critical findings.
4. Merge PR into `main` only after required checks are green.
5. Create annotated and signed tag on the merged `main` commit.
6. Publish GitHub Release notes for the tagged commit.
7. If binary artifacts are included, attach binaries + `SHA256SUMS` + signatures.
8. Announce maintenance window and expected impact.
9. Start high-frequency monitoring for:
   - delivery success rate
   - timeout/retry rates
   - abnormal rejection rates for grant/PoW validations
10. Keep rollback owner available for immediate response.

### Binary Artifact Policy (Optional)

- Binary publishing is optional and must not bypass source release checks.
- If binaries are published, include checksum and signature verification guidance.
- If binaries are not published, release notes must describe source build commands.

### CodeQL False-Positive Handling

- Never suppress without triage.
- Triage must include data-flow verification and explicit rationale.
- Dismissed findings must remain auditable in PR/release records.

## 3) Incident Triage

When incidents happen:

1. Classify severity:
   - Sev-1: security or data-integrity risk
   - Sev-2: major service degradation
   - Sev-3: minor degradation
2. Collect deterministic evidence:
   - CI status for candidate commit
   - runtime metrics snapshots
   - explicit failing test or validation path
3. Decide containment action:
   - traffic reduction
   - temporary feature gate disablement (if pre-approved)
   - rollback trigger

## 4) Recovery Procedure

1. Preserve forensic artifacts (logs, metrics, hashes).
2. Re-run critical and robustness gates on rollback target.
3. Deploy last known-good version.
4. Validate post-rollback health indicators.
5. Publish incident summary with timeline and mitigations.

## 5) Post-Incident Requirements

- Add regression test for root cause.
- Update release checklist/runbook if process gaps are found.
- Record unresolved risks in `TODO.md` and assign owners.
