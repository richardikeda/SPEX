# Security Advisory Exceptions

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

## Active Exceptions

| Advisory | Package | Scope | Approval | Expiry |
| --- | --- | --- | --- | --- |
| RUSTSEC-2026-0118 | `hickory-proto` 0.25.2 | Optional `libp2p` DNS/mDNS packages retained in `Cargo.lock` but absent from the active feature graph. | Maintainer approval recorded on 2026-07-16. | 2026-08-16 |
| RUSTSEC-2026-0119 | `hickory-proto` 0.25.2 | Optional `libp2p` DNS/mDNS packages retained in `Cargo.lock` but absent from the active feature graph. | Maintainer approval recorded on 2026-07-16. | 2026-08-16 |

## Controls and Removal Condition

- `libp2p` is declared with `default-features = false`; SPEX enables only the
  transport features used by the TCP/Noise/Yamux swarm.
- The release supply-chain gate fails if `libp2p-dns` or `libp2p-mdns` appears
  in the active Cargo feature graph.
- The exceptions suppress only the two listed audit IDs. They do not suppress
  advisories for active dependency paths or any other packages.
- Remove both exceptions immediately when a compatible `libp2p` release stops
  retaining the affected optional packages or upgrades `hickory-proto` to a
  fixed release.

Secure. Permissioned. Explicit.
