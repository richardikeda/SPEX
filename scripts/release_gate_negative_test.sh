#!/usr/bin/env bash
set -euo pipefail

# This script proves that release gates reject failing critical commands.
# Invariant: an intentionally failing command must produce non-zero exit status.

set +e
bash -lc 'exit 17' >/dev/null 2>&1
status=$?
set -e

if [[ "$status" -eq 0 ]]; then
  echo "negative gate check failed: failure was not detected" >&2
  exit 1
fi

# The expiry checker must reject a deterministic expired exception without
# modifying the normative document or relying on the runner's wall clock.
set +e
printf '%s\n' '| TEST-ADVISORY | `test` 1.0 | Fixture | Test approval | 2026-08-21 |' |
  bash scripts/check_advisory_exception_expiry.sh - 2026-08-22 >/dev/null 2>&1
expiry_status=$?
set -e

if [[ "$expiry_status" -eq 0 ]]; then
  echo "negative gate check failed: expired advisory exception was accepted" >&2
  exit 1
fi

echo "negative gate check passed (detected failure status=$status)"
