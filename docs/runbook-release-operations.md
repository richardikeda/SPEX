# Release Operations Runbook (v1.0)

This runbook consolidates operational actions for go-live, incident triage, and rollback.

## 1) Pre-Go-Live

1. Confirm all mandatory release gates passed (`docs/release-v1-checklist.md`).
2. Confirm monitoring dashboards include publish/recovery/fallback metrics.
3. Confirm on-call and incident channels are staffed.
4. Confirm backup/export path for append-only logs is available.

## 2) Go-Live Sequence

1. Tag release candidate commit internally.
2. Announce maintenance window and expected impact.
3. Publish release artifacts.
4. Start high-frequency monitoring for:
   - delivery success rate
   - timeout/retry rates
   - abnormal rejection rates for grant/PoW validations
5. Keep rollback owner available for immediate response.

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
