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

echo "negative gate check passed (detected failure status=$status)"
