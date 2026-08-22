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
| RUSTSEC-2026-0118 / GHSA-3v94-mw7p-v465 | `hickory-proto` 0.25.2 | Optional `libp2p` DNS/mDNS packages retained in both lockfiles but absent from the active feature graph. | Renewed by the maintainer on 2026-08-22 after the prior approval expired on 2026-08-16. | 2026-09-22 |
| RUSTSEC-2026-0119 / GHSA-q2qq-hmj6-3wpp | `hickory-proto` 0.25.2 | Optional `libp2p` DNS/mDNS packages retained in both lockfiles but absent from the active feature graph. | Renewed by the maintainer on 2026-08-22 after the prior approval expired on 2026-08-16. | 2026-09-22 |
| GHSA-vxx9-2994-q338 | `yamux` 0.12.1 | `libp2p-yamux` 0.47.0 retains a legacy 0.12 backend, but its default configuration selects patched `yamux` 0.13.10. SPEX uses only that default configuration. | Renewed by the maintainer on 2026-08-22. | 2026-09-22 |
| AIKIDO-2026-850672 | `tls_codec` 0.4.2 | Scanner-specific finding retained only through optional `x509-cert` metadata. `cargo tree --locked -i tls_codec@0.4.2` has no active path, including the fuzz workspace. | Renewed by the maintainer on 2026-08-22. | 2026-09-22 |
| RUSTSEC-2024-0436 | `paste` 1.0.15 | Unmaintained transitive dependency in the active Linux libp2p TCP interface-discovery graph. No vulnerability is reported and no compatible upstream replacement exists. | Renewed by the maintainer on 2026-08-22. | 2026-09-22 |

## Review Evidence — 2026-08-22

- Root and fuzz lockfiles were audited separately. Both initially contained
  vulnerable `h2` 0.4.15 and unsound `event-listener` 5.4.1 paths; they now
  resolve patched `h2` 0.4.16 and `event-listener` 5.4.2.
- `cargo tree --workspace --locked --all-features --target all` confirms that
  `hickory-proto` 0.25.2 and `tls_codec` 0.4.2 have no active SPEX path.
- The same target-wide graph confirms that `paste` remains active only through
  the Linux `libp2p-tcp` interface watcher and that both Yamux versions remain
  transitive dependencies of `libp2p-yamux` 0.47.0.
- The release documentation gate enforces the Yamux 0.13 default constructor
  and rejects every compatibility API that would switch execution to 0.12.
- No newer compatible `libp2p`, `libp2p-yamux`, or `if-watch` release was
  available at review time. Forced major-version substitutions were rejected.
- The two Hickory exceptions were not backdated: their expired interval from
  2026-08-17 through 2026-08-22 remains explicit in this record.

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
- `libp2p-yamux` 0.47.0 uses `yamux` 0.13 by default. Calling any legacy
  configuration setter switches it to 0.12; the release documentation gate
  rejects those APIs and requires the audited default constructor.
- Remove the Yamux exception as soon as a compatible `libp2p-yamux` release no
  longer retains 0.12. Reassess immediately if SPEX needs a custom Yamux
  configuration before then.
- Do not force `tls_codec` 0.5 into the graph: the 0.4 entry is an inactive
  transitive API selected by upstream version constraints. Remove this
  exception when the `mls-rs`/`x509-cert` dependency chain upgrades or when a
  public advisory supplies a compatible remediation.
- Track the unmaintained `paste` warning on every libp2p update and remove the
  exception when `if-watch`/`netlink-packet-core` no longer depends on it.
- The fuzz lockfile is regenerated from the current manifests and audited
  independently. This updates `quinn-proto` from 0.11.13 to patched 0.11.16 and
  `yamux` from 0.13.8 to patched 0.13.10.
- The release documentation gate rejects any active-exception row whose ISO
  expiry date is earlier than the current UTC date.

## Resolved Findings in This Review

| Finding | Resolution |
| --- | --- |
| GHSA-4w2j-m93h-cj5j (`quinn-proto`) | Root and fuzz lockfiles resolve 0.11.16, above the patched minimum 0.11.15. |
| Stale fuzz dependency graph | `fuzz/Cargo.lock` was regenerated and now passes `cargo audit --file fuzz/Cargo.lock`. |
| Unpinned GitHub Actions | Every remote action is pinned to a reviewed 40-character commit SHA and the release gate rejects mutable refs. |
| Broad release token permission | Workflow default is `contents: read`; only the release-creation job has job-scoped `contents: write`. |
| RUSTSEC-2026-0258 (`h2`) | Root and fuzz lockfiles resolve patched 0.4.16 instead of vulnerable 0.4.15. |
| RUSTSEC-2026-0221 (`event-listener`) | Root and fuzz lockfiles resolve patched 5.4.2 instead of unsound 5.4.1. |

Secure. Permissioned. Explicit.
