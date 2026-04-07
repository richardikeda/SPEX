# SPEX Manifesto

## Protocol Alignment (Normative)

SPEX means **Secure Permissioned Exchange**.
SPEX is a **protocol**, not just an application.
Security comes before convenience.
Core cryptographic invariants are non-negotiable.
All architecture and behavior described in this document must remain aligned with:
**Secure. Permissioned. Explicit.**

Secure Permissioned Exchange

## 1. Communication Is Critical Infrastructure

Communication is not a side feature.
It is critical infrastructure for people, organizations, and systems.

Many real-world failures happen because communication is implicit, over-permissive, and poorly bounded.
SPEX exists to make communication explicit, verifiable, and constrained.

---

## 2. Communication Is a Cryptographic Act

In SPEX, a message is not just bytes from A to B.
It includes explicit identity, permissions, time constraints, and verifiable context.

Communication without context becomes noise.
Communication without boundaries becomes risk.

---

## 3. Security Means Verification, Not Trust

SPEX follows one core rule:

Do not trust transport, server, or network. Verify everything.

Therefore:

- bridges and DHT paths are untrusted
- message processing validates hash, signature, and context
- key changes are critical events
- expiration and revocation are mandatory controls

---

## 4. Permissions Must Be Explicit

Implicit permissions are rejected.

In SPEX, permissions are:

- explicitly granted
- cryptographically verifiable
- revocable
- time-bounded

Without explicit permission, communication is denied.

---

## 5. Anti-Spam Is Economic Friction

SPEX uses asymmetric communication cost:

- verifiable proof-of-work
- explicit limits per grant

Initiation costs more than receiving, making abuse economically harder.

---

## 6. Small Trusted Core, Flexible Edges

SPEX does not invent cryptography.
It composes proven primitives, canonical serialization, MLS, and pluggable transport.

The core must remain small, auditable, and stable.

---

## 7. Transport Can Change, Contract Cannot

SPEX can run over P2P, DHT, or HTTP bridges.
Transport may vary; cryptographic contract invariants may not.

---

## 8. Privacy Means Control

SPEX does not promise universal anonymity.
It provides control, metadata minimization, explicit consent, and clear boundaries.

---

## 9. SPEX Is Intentionally Specialized

SPEX is built for serious communication in critical or sensitive contexts.
It is not designed to be a generic social messaging platform.

---

## 10. Open Source with Shared Responsibility

SPEX is open source because auditability and transparency are security requirements.

Open source does not remove responsibility.
Users and contributors must preserve invariants and avoid convenience-driven security regressions.

---

## 11. Communication as Contract

Core statement:

Communication is a cryptographic contract between parties, not an informal data stream.

This mindset reduces risk and improves operational clarity.

---

## Why SPEX Exists

SPEX exists because communication deserves serious engineering.

Institutional note:

- SPEX was created in 2026 by Richard Ikeda.
- It was built with extensive AI and testing tooling, plus technical engineering effort.
- It began as a personal-use protocol and is open source for adoption and code auditing.

**Secure.
Permissioned.
Explicit.**
