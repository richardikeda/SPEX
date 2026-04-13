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
  ".github/branch-protection/main.json"
)

for file in "${required_files[@]}"; do
  if [[ ! -f "$file" ]]; then
    echo "missing required release document: $file" >&2
    exit 1
  fi
done

if ! grep -Eq "docs/release-v1-checklist\.md" README.md; then
  echo "README.md must reference docs/release-v1-checklist.md" >&2
  exit 1
fi

if ! grep -Eq "docs/runbook-release-operations\.md" README.md; then
  echo "README.md must reference docs/runbook-release-operations.md" >&2
  exit 1
fi

if ! grep -Eq "\[TASK 1\]|\[TASK 2\]|\[TASK 3\]" TODO.md; then
  echo "TODO.md must keep explicit remaining blockers for v1.0 closure" >&2
  exit 1
fi


if ! grep -Eq "release-critical-tests" .github/workflows/release-readiness.yml; then
  echo "release-readiness workflow must define release-critical-tests job" >&2
  exit 1
fi

if ! grep -Eq "cargo test -p spex-core --locked --all-features" .github/workflows/release-readiness.yml; then
  echo "release-critical-tests must test spex-core" >&2
  exit 1
fi

if ! grep -Eq "cargo test -p spex-mls --locked --all-features" .github/workflows/release-readiness.yml; then
  echo "release-critical-tests must test spex-mls" >&2
  exit 1
fi

if ! grep -Eq "cargo test -p spex-transport --locked --all-features" .github/workflows/release-readiness.yml; then
  echo "release-critical-tests must test spex-transport" >&2
  exit 1
fi

if ! grep -Eq 'CI Umbrella' .github/branch-protection/main.json; then
  echo "branch protection must require the CI Umbrella status check" >&2
  exit 1
fi

if ! grep -Eq 'Release Readiness / Critical release tests \(core, mls, transport\)' .github/branch-protection/main.json; then
  echo "branch protection must require the critical release tests status check" >&2
  exit 1
fi

if ! grep -Eq 'Version Guard / version-bump-required' .github/branch-protection/main.json; then
  echo "branch protection must require the version guard status check" >&2
  exit 1
fi

echo "release documentation gate passed"
