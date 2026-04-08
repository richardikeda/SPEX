# 0001 — License: Mozilla Public License 2.0

**Date:** 2026-04-08
**Status:** Accepted
**Author:** @richardikeda

## Context

SPEX is a security-critical messaging and transport protocol published as open source.
The license choice directly affects:

- Whether companies and developers can adopt SPEX in commercial and proprietary products.
- Whether modifications to SPEX core files must be published.
- Whether someone can operate a SaaS built on SPEX without releasing source code.
- The legal risk profile for potential contributors.

Two candidates were evaluated:

| Attribute | MPL-2.0 | AGPLv3 |
|---|---|---|
| Copyleft scope | File-level | Program + network use |
| Proprietary products using SPEX unmodified | Allowed | Requires AGPL-compatible release |
| SaaS using SPEX unmodified | Allowed | Triggers network-use clause |
| Modifications to SPEX files must be published | Yes | Yes (stricter) |
| Corporate adoption barrier | Low | High |
| Defense against "SaaS parasitism" | Partial | Strong |
| OSI-approved | Yes | Yes |

## Decision

SPEX is licensed under **Mozilla Public License 2.0 (MPL-2.0)**.

The protocol core (`spex-core`, `spex-mls`, `spex-transport`, `spex-bridge`,
`spex-client`, `spex-cli`) is a single-license MPL-2.0 project.
There is no AGPL clause, no dual licensing, and no commercial licensing exception
in the initial release.

## Rationale

The primary objective for the initial open source release is **broad adoption
and external security review**, not revenue protection.

MPL-2.0 satisfies both:
- File-level copyleft ensures that improvements to SPEX source files flow back to
  the community.
- Permissive composition allows companies, integrators, and SDK consumers to embed
  SPEX in their products without full GPL compliance overhead.
- Contributors face a low legal friction profile: no CLA required, no corporate
  legal review of AGPL network-use clause needed.

### Why not AGPLv3

AGPLv3 would prevent unattributed SaaS exploitation more strongly, but at a
significant adoption cost:
- Most enterprise legal departments treat AGPL as a "do not use" signal without
  explicit OSS counsel review and waiver.
- The goal of distributing a protocol widely for security review outweighs the
  risk of SaaS parasitism at this stage.
- If enterprise SaaS exploitation becomes a concrete problem, a future ADR can
  propose dual licensing (MPL-2.0 + commercial) as a revenue option without
  re-licensing the OSS track.

### Future dual licensing

MPL-2.0 is compatible with a future dual licensing model where:
- OSS/community use remains MPL-2.0.
- Enterprise/hosted/SaaS products operate under a separate commercial license.

This ADR does not preclude that path; it simply does not activate it.

## Consequences

- All published SPEX files carry `// SPDX-License-Identifier: MPL-2.0` headers.
- A `NOTICE` file at the repository root documents copyright, license summary,
  and third-party attribution guidance.
- `LICENSE.MD` at the repository root contains the full MPL-2.0 text.
- SaaS operators may use SPEX as-is without license obligation, but file-level
  modifications to SPEX source must be published under MPL-2.0.
- A future ADR is required before any dual licensing or re-licensing decision.
