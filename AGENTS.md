# AGENTS.md

Rules, task template, and PR template for AI agents in the SPEX project.

## Purpose

This document defines how AI agents must operate when contributing to SPEX.

SPEX is a security-critical protocol.
AI agents are allowed to assist, but never to bypass explicit design, security, testing, or review requirements.

Project north:
- SPEX means Secure Permissioned Exchange.
- SPEX is a protocol, not just an application.
- Security comes before convenience.
- Core cryptographic invariants are non-negotiable.
- Final principle: Secure. Permissioned. Explicit.

## 1. Mandatory Normative Compliance

For every task, agents MUST read, respect, and stay consistent with:
- CODE_OF_CONDUCT.md
- CONTRIBUTING.md
- MANIFEST.md
- SECURITY.md
- TESTS.md
- all documents under docs/

If any instruction conflicts with these documents, the agent MUST stop, report the conflict, and request human clarification.

Agents MUST NOT create cross-document inconsistency in objective, protocol identity, architecture direction, or security guarantees.

## 2. Core Principles

All AI agents MUST operate under these principles:
- Security first
- Explicitness over convenience
- Deterministic behavior
- No silent assumptions
- Human accountability

If a task conflicts with these principles, the agent must stop and report.

## 3. Hard Requirements

### 3.1 Versioning

- Increment VERSION.md for every executed task.
- Rule:
  - Increment last position from 1 to 99.
  - After 99, increment middle position by 1 and reset last position to 0.
  - Example: 0.0.99 -> 0.1.0.

### 3.2 Code Quality

- All functions and methods MUST have English comments explaining:
  - what the function does
  - important invariants or constraints
- Code must be readable, deterministic, and explicit.

### 3.3 Tests

- Every implementation or fix MUST include corresponding test(s).
- Tests MUST be executed before commit.
- Include negative cases when applicable.
- Security-relevant changes require explicit security tests.

### 3.4 Documentation

- If APIs or behavior change:
  - update README.md
  - update relevant files under docs/
- Documentation updates are mandatory.
- Documentation must remain aligned with project north and protocol invariants.

### 3.5 Security Expectations

Agents MUST:
- validate grants and permissions explicitly
- enforce minimum PoW requirements where applicable
- avoid weakening authentication, authorization, or validation logic
- preserve protocol invariants (CBOR canonical, signatures, cfg_hash, epoch)

If a task would weaken security, the agent must refuse to proceed.

### 3.6 Robustness Strategy

- Parsing/decoding boundaries should include fuzz targets when feasible.
- Invariants should include property-based tests for determinism/idempotence and malformed input handling.
- Untrusted input must return explicit errors and never depend on panic paths.
- New critical parsers should expose stable test entry points for fuzzing.

## 4. Allowed and Forbidden Actions

Agents MAY:
- implement clearly specified tasks
- refactor without behavior changes
- add tests and improve coverage
- improve documentation clarity
- improve explicit error messages
- generate CI/config/templates
- perform mechanical repetitive changes

Agents MUST NOT:
- redesign cryptography
- change wire format without explicit approval
- relax validation rules
- add implicit behavior
- introduce insecure defaults
- merge changes without tests

## 5. Mandatory Task Template and Approval Gate

Every task handled by an AI agent MUST use this structure.

If the requester does not provide the template, the agent MUST:
1. Convert the request into this template.
2. Fill all fields with available information.
3. Mark missing items as [NEEDS CLARIFICATION].
4. Send the completed template back to the requester.
5. Explicitly ask for execution approval (example: Can I proceed with execution?).

The agent MUST NOT execute the task until explicit requester approval is given.

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
- What conditions define this task as done?
- Expected behavior, including failure cases

Tests Required:
- What tests must be added or updated?
- Any negative/security test required?

Documentation:
- Which docs must be updated (README, docs/*)?

Versioning:
- Confirm that VERSION.md must be incremented

If any section remains ambiguous, the agent MUST ask for clarification and wait for confirmation before proceeding.

## 6. Pull Request Template (Mandatory)

All agent-assisted pull requests MUST include:

## What
Describe clearly what this PR does.

## Why
Explain the motivation and the problem being solved.
Reference issues or design notes if applicable.

## Changes
- List concrete changes
- List files/modules affected

## Security Impact
- Does this change affect security? (Yes/No)
- If yes:
  - explain impact
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
- Areas that need special attention

PRs missing any of these sections are incomplete.

## 7. Stop Conditions

An agent MUST stop and report if:
- requirements are ambiguous
- task implies protocol change without explicit approval
- task weakens security controls
- tests cannot reasonably be added
- documentation impact is unclear
- a shortcut would bypass validation
- requester approval was not explicitly granted after the filled task template was returned
- requested change would create inconsistency with CODE_OF_CONDUCT.md, CONTRIBUTING.md, MANIFEST.md, SECURITY.md, TESTS.md, or docs/*

Silence or guessing is unacceptable.

## 8. Review and Accountability

- All agent-generated changes require human review.
- Agents must not merge code autonomously.
- Suggested commit footer:

Assisted-by: AI agent (reviewed by maintainer)

Responsibility always remains with human maintainers.

## 9. Final Rule

If an AI agent is unsure whether a change is correct, safe, allowed, or consistent:

Stop. Ask. Do not guess.

SPEX values correctness, security, and clarity over speed or automation.

Secure.
Permissioned.
Explicit.
