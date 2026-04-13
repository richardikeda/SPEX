# SPEX v1.0.0 Go/No-Go Decision Record

Date: 2026-04-07
Candidate version: 1.0.0
Candidate commit SHA: 79bf6ca
Reviewer: AI-assisted preparation (human maintainer approval required)

## Gate Results

- Gate A (critical tests): PASS
  - Command: `cargo test -p spex-core -p spex-mls -p spex-transport --locked --all-features --verbose`
- Gate B (security/robustness regression): PASS
  - Command: `cargo test --workspace --locked -q`
- Gate C (documentation/quality): PASS
  - Command: `cargo fmt --all -- --check`
  - Command: `cargo deny check`
- Negative gate (block-on-failure behavior): PASS (validated equivalent behavior via explicit non-zero exit in shell)

## Decision

GO

## Notes

- `cargo deny check` result: `advisories ok, bans ok, licenses ok, sources ok`.
- Remaining temporary risk exception is explicit and documented in policy:
  - `RUSTSEC-2021-0127` (`serde_cbor`, unmaintained) is tracked as temporary exception in `deny.toml` pending migration.
- All workspace tests passed at validation time.
