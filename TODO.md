# SPEX TODO List (v1.0.4)

## Scope

All v1.0 code blockers are resolved. The items below require maintainer action
before or at the time of public deployment.

---

## [RELEASE GATE] Push Git Version Tags to Origin

Local tags exist: `v1.0.0`, `v1.0.1`, `v1.0.2`, `v1.0.3`, `v1.0.4`.
Only `v1.0.1` is currently on the remote.

Required action (maintainer):
```bash
git push origin v1.0.0 v1.0.2 v1.0.3 v1.0.4
```

Acceptance criteria:
- All tags visible on the remote (GitHub releases / tags page).
- `CHANGELOG.md` "Published Versions" section matches.

---

## Post-Release Backlog (Not Blocking)

- Run `./scripts/tls_validation.sh <bridge-host>` before first production deployment
  and attach `tls-validation-evidence.txt` to the release notes.
- Longer-duration transport churn/soak campaigns and expanded anti-eclipse thresholds.
- Advanced MLS cross-implementation interop matrix expansion.
- Stateful and differential fuzz campaign expansion beyond release smoke baseline.
- Broader CI matrix expansion (Windows/macOS full test execution).
- Observability exporter standardization and dashboard packaging.
