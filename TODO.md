# SPEX TODO List (v1.0.4)

## Scope

All v1.0 blockers and post-1.0 backlog items have been resolved.
The only remaining actions require maintainer authorization (git push, live TLS deployment).

---

## [RELEASE GATE] Push Git Version Tags to Origin

Local tags have been created (`git tag --list` returns v1.0.0, v1.0.1, v1.0.2, v1.0.3, v1.0.4).
They must be pushed to the remote repository to establish public release provenance.

Required action (maintainer):
```bash
git push origin v1.0.0 v1.0.1 v1.0.2 v1.0.3 v1.0.4
```

Acceptance criteria:
- Tags are visible on the remote (e.g., GitHub releases / tags page).
- `CHANGELOG.md` "Published Versions" section lists v1.0.4 as current.

---

## [RELEASE GATE] Run TLS Validation and Attach Evidence

Automates the checklist from `docs/bridge-tls-deployment.md`.

Required action (maintainer):
```bash
./scripts/tls_validation.sh <your-bridge-host>
# Attach tls-validation-evidence.txt to the v1.0.4 release.
```

Acceptance criteria:
- Script exits 0 (all checks pass).
- Evidence file is attached to the release notes.

---

## Post-1.0 Backlog (Not Blocking Release)

- Longer-duration transport churn/soak campaigns and expanded anti-eclipse thresholds.
- Advanced MLS cross-implementation interop matrix expansion.
- Stateful and differential fuzz campaign expansion beyond release smoke baseline.
- Broader CI matrix expansion (for example, Windows/macOS full test execution).
- Observability exporter standardization and dashboard packaging.
