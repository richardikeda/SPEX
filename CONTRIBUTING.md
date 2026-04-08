# Contributing to SPEX

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

Thank you for considering contributing to SPEX.

SPEX is a security-critical protocol project focused on correctness, explicit behavior, and auditability.

---

## Code of Conduct

This project follows [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
Be respectful, evidence-driven, and technical.

---

## Project Philosophy

- SPEX is a protocol, not just an application.
- Security comes before convenience.
- Core cryptographic invariants are non-negotiable.
- Behavior must remain explicit and deterministic.

Contributions that weaken security guarantees are not accepted.

---

## What to Contribute

### Accepted Contributions

- bug fixes
- documentation improvements
- additional tests
- ergonomics improvements that do not reduce security
- performance improvements that preserve semantics
- tooling around protocol usage (CLI/SDK/examples)

### Avoid

- adding dependencies without strong justification
- implicit or magic behavior
- relaxing security validation
- undocumented wire format changes
- hype-driven features without protocol value

---

## Development Setup

### Requirements

- stable Rust toolchain
- cargo

### Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

PRs with failing CI are not accepted.

---

## Repository Structure

Main crates:

* `spex-core`
  Cryptographic primitives, core types, validation, invariants.

* `spex-mls`
  MLS integration and SPEX extensions.

* `spex-transport`
  P2P transport, chunking, HTTP fallback.

* `spex-bridge`
  Untrusted HTTP bridge with explicit validation.

* `spex-client`
  High-level SDK for applications.

* `spex-cli`
  Reference CLI application.

---

## Security-Sensitive Changes

Changes that affect:

- cryptography
- CBOR serialization
- hashing
- grant validation
- MLS / ratchet behavior
- anti-abuse controls (PoW)

must:

- be explicitly identified in the PR
- include technical rationale
- include tests
- update relevant documentation

These changes receive stricter review.

---

## Commit Guidelines

- keep commits small and focused
- use clear imperative commit messages
- avoid vague commit names like misc, wip, or fix stuff

Examples:

```
Add grant expiration validation
Harden PoW parameter checks
Document bridge inbox endpoint
```

---

## Pull Request Process

1. Fork the repository.
2. Create a descriptive branch:

   ```
   feature/grant-capabilities
   fix/pow-validation
   docs/bridge-api
   ```
3. Ensure all tests pass.
4. Update documentation where needed.
5. Open a PR describing:

  - the problem
  - the solution
  - security impact (if any)
6. Sign off each commit with Developer Certificate of Origin attestation:

    ```
    git commit -s
    ```

     The sign-off certifies that you have the right to submit the contribution under this repository's license.

  ---

  ## Versioning

  - `Cargo.toml` is authoritative for publishable crate versions and SemVer decisions.
  - `VERSION.md` represents protocol/repository release metadata, not per-crate publish version.
  - `CHANGELOG.md` must document notable changes for each repository release.
  - Do not force crate version harmonization without an explicit publish audit and release decision.

---

## Documentation Requirements

If your PR:

- changes behavior -> update docs
- changes wire format -> update docs/wire-format.md
- changes APIs -> update examples

Documentation is mandatory.

---

## Release Readiness (v1.0)

Any contribution affecting release closure must preserve these reproducible gates:

```bash
cargo test --workspace --locked --verbose
cargo test --workspace --locked --all-features --verbose
cargo fmt --all -- --check
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
./scripts/release_gate_docs.sh
./scripts/release_gate_negative_test.sh
```

These gates are mandatory for go/no-go decisions and must stay stable in CI.

---

## Licensing

By contributing, you agree that:

- your contribution is licensed under Mozilla Public License 2.0
- you have the legal right to contribute the submitted code
- your commits include a DCO sign-off (`Signed-off-by:` line)

SPEX currently uses a single open source license: MPL-2.0.
This repository is not dual-licensed and does not currently adopt AGPL terms.

---

## Questions and Discussion

For general questions:

- use issues
- provide technical context
- keep reports explicit and reproducible

For security reports:

- do not open public issues
- follow SECURITY.md disclosure guidance

---

## Final Notes

SPEX does not aim to be the largest project.
It aims to be correct, auditable, and trustworthy.

Contribute carefully.

---

**Secure.
Permissioned.
Explicit.**
