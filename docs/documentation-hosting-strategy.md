# Documentation Hosting Strategy

## Scope

This document defines how SPEX documentation is published for public audiences
while preserving protocol security and maintenance discipline.

## Current Decision (v1.x)

Use GitHub Pages with MkDocs as the primary public documentation host.

Rationale:

- professional public presentation
- strong navigation/search support
- low operational overhead
- no additional third-party hosting dependency for protocol-critical docs

## Read the Docs Status

Read the Docs is deferred for now.

It may be revisited when objective triggers are met:

- two or more active documentation versions must be supported in parallel
- multilingual documentation expands beyond the current baseline
- the maintenance cost/benefit becomes favorable

## Security and Governance Constraints

- Security policy remains authoritative in `SECURITY.md`.
- Protocol invariants remain authoritative in `MANIFEST.md` and related specs.
- Documentation publication must not weaken disclosure, validation, or release gates.

## Review Cadence

Reassess hosting strategy at each minor roadmap review or when external adoption
creates a clear versioning/i18n requirement.
