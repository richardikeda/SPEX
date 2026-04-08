# SPEX TODO List (v1.0.5)

## Scope

All v1.0 code blockers are resolved. The items below are pending actions
organized by priority.

---

## Release Gates (Maintainer Action Required)

### [TASK 1] Push Git Version Tags to Origin

Local tags exist: `v1.0.0`–`v1.0.5`.
Only `v1.0.1` is currently on the remote.

Required action (maintainer):
```bash
git push origin v1.0.0 v1.0.2 v1.0.3 v1.0.4 v1.0.5
```

Acceptance criteria:
- All tags visible on the remote (GitHub releases / tags page).
- `CHANGELOG.md` "Published Versions" section matches.

---

## [TASK 2] Pre-Production Validation

- [ ] Run `./scripts/tls_validation.sh <bridge-host>` before first production deployment
  and attach `tls-validation-evidence.txt` to the release notes.

---

## [TASK 3] Post-Release Backlog

### Testing & Fuzzing
- [ ] Longer-duration transport churn/soak campaigns and expanded anti-eclipse thresholds.
- [ ] Advanced MLS cross-implementation interop matrix expansion.
- [ ] Stateful and differential fuzz campaign expansion beyond release smoke baseline.

### CI & Infrastructure
- [ ] Broader CI matrix expansion (Windows/macOS full test execution).

### Observability
- [ ] Observability exporter standardization and dashboard packaging.
