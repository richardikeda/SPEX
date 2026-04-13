# SPEX Roadmap

SPEX is a security-first messaging and transport protocol.
This roadmap documents the planned evolution from the current v1.0 release baseline
toward v2 and beyond.

Roadmap items are not binding commitments. Protocol invariants and security properties
are never sacrificed for schedule.

---

## Current State — v1.0 (released)

All v1.0 code blockers are resolved. The release includes:

- `spex-core`: canonical CBOR (CTAP2), signing, hashing, PoW, shared validation types.
- `spex-mls`: MLS integration, epoch safety, and recovery paths.
- `spex-transport`: P2P transport, chunking/manifests, fallback paths, ingestion validation.
- `spex-bridge`: HTTP relay with explicit validation and abuse controls.
- `spex-client`: high-level SDK for identity, state, thread, and message flows.
- `spex-cli`: reference CLI for operational and integration use.

Pending post-release operational tasks are tracked in [TODO.md](TODO.md).

---

## Near-term — Maintenance (v1.x)

These are improvements that do not require breaking changes to the wire format
or public API:

- [ ] Expand CI matrix to full Windows and macOS test execution.
- [ ] Longer-duration transport soak and anti-eclipse test campaigns.
- [ ] Stateful and differential fuzz expansion beyond the release smoke baseline.
- [ ] Observability exporter standardization.
- [ ] TLS validation evidence attached to the first production deployment release notes.
- [ ] First publish of crates to crates.io (requires publish audit — see [TODO.md](TODO.md)).

---

## Medium-term — Protocol v2 (planned)

These items are under design consideration and will require explicit review,
protocol version change, and documented Architecture Decision Records (ADRs)
before any implementation starts.

- [ ] Evaluate AGPL vs MPL-2.0 dual licensing for hosted/SaaS deployment contexts.
      See [docs/decisions/0001-license-mpl-2.md](docs/decisions/0001-license-mpl-2.md).
- [ ] Cross-implementation MLS interoperability matrix expansion with external clients.
- [ ] Wire format v2 revision with explicit version negotiation header.
- [ ] P2P discovery hardening beyond the current Kademlia baseline.
- [ ] Formal threat model document reviewed by independent security researcher.

---

## Long-term — Ecosystem

These items depend on v2 stability and external adoption signals:

- [ ] Reference bridge deployment guide as infrastructure-as-code (IaC).
- [ ] Client SDK FFI bindings for at least one additional language.
- [ ] Signed release artifacts and SBOM generation.

---

## Out of Scope (Protocol Invariants — Never)

The following will not change regardless of version, schedule, or request:

- CBOR canonical encoding (CTAP2) for all signed or hashed structures.
- Explicit grant and permission validation at all trust boundaries.
- PoW requirements where abusive behavior must be disincentivized.
- Minimum signature and key standards.

---

## Contributing to the Roadmap

Protocol-level proposals must follow contribution and review rules in
[CONTRIBUTING.md](CONTRIBUTING.md) and go through a documented ADR in
[docs/decisions/](docs/decisions/) before any
implementation is started.

Non-protocol suggestions (tooling, CI, documentation) can be raised as issues
using the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md).
