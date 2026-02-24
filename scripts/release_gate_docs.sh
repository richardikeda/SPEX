#!/usr/bin/env bash
set -euo pipefail

# This script validates release-readiness documentation invariants required for v1.0 gates.
# Invariants: mandatory files exist and README references the release checklist/runbook.

required_files=(
  "README.md"
  "CHANGELOG.md"
  "CONTRIBUTING.md"
  "docs/release-v1-checklist.md"
  "docs/runbook-release-operations.md"
)

for file in "${required_files[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "missing required release document: $file" >&2
    exit 1
  fi
done

if ! rg -q "docs/release-v1-checklist\.md" README.md; then
  echo "README.md must reference docs/release-v1-checklist.md" >&2
  exit 1
fi

if ! rg -q "docs/runbook-release-operations\.md" README.md; then
  echo "README.md must reference docs/runbook-release-operations.md" >&2
  exit 1
fi

if ! rg -q "\[TASK 1\]|\[TASK 2\]|\[TASK 3\]" TODO.md; then
  echo "TODO.md must keep explicit remaining blockers for v1.0 closure" >&2
  exit 1
fi

echo "release documentation gate passed"
