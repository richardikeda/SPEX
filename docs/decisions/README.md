# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for SPEX.

An ADR documents a significant architectural or protocol-level decision:
what was decided, why, what alternatives were rejected, and what constraints apply.

ADRs are **immutable once accepted** — if a decision changes, a new ADR supersedes
the old one rather than replacing it in place.

## Format

Each ADR file follows this naming convention:

```
NNNN-short-title.md
```

Where `NNNN` is a zero-padded four-digit sequence number starting at `0001`.

## Status Values

| Status | Meaning |
|---|---|
| Proposed | Under discussion; not yet accepted |
| Accepted | Approved and in effect |
| Superseded by NNNN | Replaced by a newer ADR |
| Deprecated | No longer relevant; no replacement |

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-license-mpl-2.md) | License: Mozilla Public License 2.0 | Accepted |

## Process

1. Copy the template below into a new file `NNNN-short-title.md`.
2. Fill all sections.
3. Open a PR with the ADR as the only content change.
4. After review and merge, update the index table above.

---

## Template

```markdown
# NNNN — Title

**Date:** YYYY-MM-DD
**Status:** Proposed | Accepted | Superseded by NNNN | Deprecated
**Author:** @handle

## Context

What is the situation that requires a decision?
What forces are at play (technical constraints, protocol invariants, licensing, etc.)?

## Decision

State the decision clearly in one or two sentences.

## Rationale

Why is this the best option, given the context?
What alternatives were considered and why were they rejected?

## Consequences

What becomes easier or harder as a result?
What invariants must be preserved?
What monitoring or follow-up is needed?
```
