# AGENTS.md

**Rules, Task Template, and PR Template for AI Agents in the SPEX Project**

## Purpose

This document defines **how AI agents must operate** when contributing to the SPEX project.

SPEX is a **security-critical protocol**.
AI agents are allowed to assist, but **never to bypass explicit design, security, or review requirements**.

Agents must follow these rules strictly.

---

## 1. Core Principles

All AI agents MUST operate under the following principles:

* **Security first**
* **Explicitness over convenience**
* **Deterministic behavior**
* **No silent assumptions**
* **Human accountability**

If a task conflicts with these principles, the agent must **stop and report**.

---

## 2. Mandatory Contribution Rules (Hard Requirements)

These rules are **non-negotiable** and must be followed in every task:

### 2.1 Versioning

* Increment the version counter in `VERSION.md` **for every executed task**.
* Versioning rule:

  * Increment the **last position** from `1` to `99`.
  * After reaching `99`, increment the **middle position by 1** and reset the last position to `0`.
  * Example: `0.0.99 → 0.1.0`.

---

### 2.2 Code Quality

* All functions and methods MUST have **English comments** explaining:

  * what the function does
  * important invariants or constraints
* Code must be:

  * readable
  * deterministic
  * explicit

---

### 2.3 Tests

* Every implementation or fix MUST be accompanied by corresponding test(s).
* Tests MUST:

  * be executed before commit
  * include negative cases when applicable
* Security-relevant changes require **explicit security tests**.

---

### 2.4 Documentation

* If APIs change:

  * update `README.md`
  * update any relevant file under `/docs`
* Documentation updates are **mandatory**, not optional.

---

### 2.5 Security Expectations

Agents MUST:

* validate grants and permissions explicitly
* enforce minimum Proof-of-Work requirements where applicable
* avoid weakening authentication, authorization, or validation logic
* preserve protocol invariants (CBOR canonical, signatures, cfg_hash, epoch)

If a task would weaken security, the agent must **refuse to proceed**.

---


### 2.6 Testing Strategy (Robustness)

* Parsing/decoding boundaries must include fuzz targets whenever feasible.
* Invariants must include property-based tests for idempotence/determinism and malformed-input handling.
* External/untrusted input must never rely on panic paths; return explicit errors.
* New critical parsers should expose stable test entrypoints to enable fuzzing.

## 3. What AI Agents Are Allowed to Do

AI agents MAY:

* implement clearly specified tasks
* refactor code **without changing behavior**
* add tests and improve coverage
* improve documentation clarity
* improve error messages (without hiding failures)
* generate CI/config/templates
* perform mechanical or repetitive changes

AI agents MUST NOT:

* redesign cryptography
* change wire formats
* relax validation rules
* add implicit behavior
* introduce insecure defaults
* merge changes without tests

---

## 4. TASK TEMPLATE (Mandatory)

Every task given to an AI agent MUST follow this structure.

Agents MUST refuse tasks that do not follow this template.

```
[TASK NAME]

Objective:
- What exactly needs to be implemented or changed?

Context:
- Why is this needed?
- Which problem does it solve?

Scope:
- Files/modules that may be modified
- Explicitly list what must NOT be touched

Constraints:
- Security invariants that must be preserved
- Performance or compatibility constraints

Acceptance Criteria:
- What conditions define this task as "done"?
- Expected behavior (including failure cases)

Tests Required:
- What tests must be added or updated?
- Any negative/security test required?

Documentation:
- Which docs must be updated (README, /docs/*)?

Versioning:
- Confirm that VERSION.md must be incremented
```

If any section is missing or ambiguous, the agent MUST ask for clarification before proceeding.

---

## 5. PULL REQUEST TEMPLATE (Mandatory)

All agent-assisted pull requests MUST include the following sections.

Agents MUST generate this content when preparing a PR.

```
## What
Describe clearly what this PR does.

## Why
Explain the motivation and the problem being solved.
Reference issues or design notes if applicable.

## Changes
- List of concrete changes
- Files/modules affected

## Security Impact
- Does this change affect security? (Yes/No)
- If yes:
  - explain the impact
  - explain how invariants are preserved
  - list validations added or modified

## Tests
- List tests added or updated
- Confirm tests were executed locally

## Documentation
- List documentation files updated
- If none, explain why no update was needed

## Versioning
- New version in VERSION.md: X.Y.Z

## Notes for Reviewers
- Anything reviewers should pay special attention to
```

PRs missing any of these sections are considered **incomplete**.

---

## 6. Stop Conditions (Agents MUST Halt)

An agent MUST stop and report if:

* requirements are ambiguous
* task implies a protocol change
* task weakens security controls
* tests cannot reasonably be added
* documentation impact is unclear
* a shortcut would bypass validation

Silence or guessing is unacceptable.

---

## 7. Review and Accountability

* All agent-generated changes require **human review**.
* Agents must not merge code autonomously.
* Suggested commit footer:

```
Assisted-by: AI agent (reviewed by maintainer)
```

Responsibility always remains with human maintainers.

---

## 8. Final Rule

If an AI agent is unsure whether a change is correct, safe, or allowed:

> **Stop. Ask. Do not guess.**

SPEX values **correctness, security, and clarity** over speed or automation.

---

**Secure.
Permissioned.
Explicit.**
