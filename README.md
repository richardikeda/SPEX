# SPEX

Secure Permissioned Exchange.

[![Rust CI](https://github.com/richardikeda/SPEX/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/richardikeda/SPEX/actions/workflows/rust.yml)
[![Release Readiness](https://github.com/richardikeda/SPEX/actions/workflows/release-readiness.yml/badge.svg?branch=main)](https://github.com/richardikeda/SPEX/actions/workflows/release-readiness.yml)

## Official Project Statement

SPEX is a security-first messaging and transport protocol with end-to-end encryption based on MLS.
It is network-agnostic and can operate over HTTP, P2P, or hybrid delivery paths.

Institutional metadata:

- Created in 2026.
- Authored by Richard Ikeda.
- Built with extensive AI and testing tooling plus technical engineering work.
- Initially developed for personal use, and published as open source for adoption and public code verification.

## What SPEX Is

- A protocol, not just an app.
- Explicit permissioned communication.
- Deterministic wire behavior (CBOR canonical/CTAP2).
- Untrusted transport model (bridge/DHT/P2P are treated as untrusted).

Protocol north:

Secure. Permissioned. Explicit.

## Repository Components

- spex-core: core types, canonical CBOR, hashing, signatures, PoW, shared validation.
- spex-mls: MLS integration and epoch/recovery safety paths.
- spex-transport: P2P transport, chunking/manifests, fallback paths, ingestion validation.
- spex-bridge: HTTP relay with explicit validation and abuse controls.
- spex-client: high-level SDK for identity/state/thread/message flows.
- spex-cli: reference CLI for operational and integration flows.

## Open Source Governance and Security Readiness

This repository includes the standard open source governance and release controls expected for a security-sensitive protocol project:

- Code of conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- Contribution rules: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security policy/disclosure: [SECURITY.md](SECURITY.md)
- Test strategy: [TESTS.md](TESTS.md)
- Mandatory release checklist: [docs/release-v1-checklist.md](docs/release-v1-checklist.md)
- Release operations runbook: [docs/runbook-release-operations.md](docs/runbook-release-operations.md)
- Branch protection policy (declarative): [.github/branch-protection/main.json](.github/branch-protection/main.json)
- CI and release gates: [.github/workflows/rust.yml](.github/workflows/rust.yml), [.github/workflows/release-readiness.yml](.github/workflows/release-readiness.yml)

SPEX is published as open source under a single project license: Mozilla Public License 2.0.
This repository does not currently use dual licensing or AGPL terms.

## Documentation Map

- Architecture overview: [docs/overview.md](docs/overview.md)
- Integration guide: [docs/integration.md](docs/integration.md)
- CLI guide: [docs/cli.md](docs/cli.md)
- Bridge API: [docs/bridge-api.md](docs/bridge-api.md)
- Bridge TLS deployment guide: [docs/bridge-tls-deployment.md](docs/bridge-tls-deployment.md)
- Protocol security guide: [docs/security.md](docs/security.md)
- Wire format reference: [docs/wire-format.md](docs/wire-format.md)
- Transport observability: [docs/observability.md](docs/observability.md)
- Operations: abuse/revocation/recovery: [docs/operations-revocation-recovery-abuse.md](docs/operations-revocation-recovery-abuse.md)
- PT-BR user guide (single document): [docs/pt-BR-guide.md](docs/pt-BR-guide.md)

## Quick Start

Build:

```bash
cargo build --workspace
```

Test:

```bash
cargo test --workspace
```

Reference release gates:

```bash
cargo test --workspace --locked --verbose
cargo test --workspace --locked --all-features --verbose
cargo fmt --all -- --check
cargo clippy --workspace --locked -- -D warnings
./scripts/release_gate_docs.sh
./scripts/release_gate_negative_test.sh
```

## Security Notes

- Use TLS for all HTTP integrations.
- Validate grants and PoW at all trust boundaries.
- Treat key changes as critical events.
- Protect local state at rest.
- Never trust transport as a security boundary.

## Getting Help

- General usage, integration, and contribution questions: open a GitHub issue with reproducible context.
- Security reports: do not use public issues; follow [SECURITY.md](SECURITY.md).
- Project changes and security fixes are published through repository history and release notes in [CHANGELOG.md](CHANGELOG.md).

## Versioning

- Crate versions in each `Cargo.toml` are authoritative for published Rust packages.
- `VERSION.md` tracks SPEX protocol/repository release metadata.
- `CHANGELOG.md` is the human-readable history of release changes.
- Version bumps are validated on every PR by the [Version Guard workflow](.github/workflows/version-guard.yml).
- To trigger a manual version bump and changelog update, use the [Auto Version Bump workflow](.github/workflows/auto-version.yml) via `workflow_dispatch`.

## License

SPEX is licensed under Mozilla Public License 2.0. See [LICENSE.MD](LICENSE.MD).

Secure.
Permissioned.
Explicit.
