#!/usr/bin/env bash
set -euo pipefail

# This script validates release-readiness documentation invariants required for v1.0 gates.
# Invariants: mandatory files exist and README references the release checklist/runbook.

required_files=(
  "README.md"
  "CHANGELOG.md"
  "CONTRIBUTING.md"
  "docs/build-performance.md"
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

if ! grep -Eq "cargo nextest run" .github/workflows/release-readiness.yml; then
  echo "release-critical-tests must use cargo-nextest" >&2
  exit 1
fi

for package in spex-core spex-mls spex-transport; do
  if ! grep -Eq -- "-p ${package}" .github/workflows/release-readiness.yml; then
    echo "release-critical-tests must test ${package}" >&2
    exit 1
  fi
done

if ! grep -Eq "cargo test" .github/workflows/release-readiness.yml ||
  ! grep -Eq -- "--doc" .github/workflows/release-readiness.yml; then
  echo "release-critical-tests must preserve doctests outside nextest" >&2
  exit 1
fi

if ! grep -Eq "cargo-nextest@0\\.9\\.137" .github/workflows/release-readiness.yml; then
  echo "release-critical-tests must pin the approved cargo-nextest version" >&2
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

# Every remote action must use an immutable full commit SHA. Local reusable
# workflows remain relative paths and are intentionally excluded.
while IFS= read -r match; do
  reference="${match#*uses:}"
  reference="${reference%%#*}"
  reference="${reference#"${reference%%[![:space:]]*}"}"
  reference="${reference%"${reference##*[![:space:]]}"}"
  if [[ "$reference" == ./* ]]; then
    continue
  fi
  if [[ ! "$reference" =~ @[0-9a-f]{40}$ ]]; then
    echo "GitHub Action is not pinned to a full commit SHA: $match" >&2
    exit 1
  fi
done < <(grep -RInE '^[[:space:]]*(-[[:space:]]+)?uses:' .github/workflows)

# Release jobs default to read-only repository access. Only the job that creates
# the GitHub Release may receive contents:write.
if ! sed -n '1,/^jobs:/p' .github/workflows/release.yml | grep -Eq '^  contents: read$'; then
  echo "release workflow must default to contents: read" >&2
  exit 1
fi
if ! sed -n '/^  create-github-release:/,$p' .github/workflows/release.yml | grep -Eq '^      contents: write$'; then
  echo "create-github-release must declare its job-scoped contents: write permission" >&2
  exit 1
fi

# libp2p-yamux 0.47 defaults to yamux 0.13, but any legacy configurator switches
# execution to the retained yamux 0.12 compatibility backend.
legacy_yamux_api="$(
  grep -RInE \
    'yamux::Config::(client|server)|set_(receive_window_size|max_buffer_size|max_num_streams|window_update_mode)' \
    crates --include='*.rs' || true
)"
if [[ -n "$legacy_yamux_api" ]]; then
  echo "legacy yamux 0.12 configuration API is forbidden:" >&2
  echo "$legacy_yamux_api" >&2
  exit 1
fi
if ! grep -Rq 'libp2p::yamux::Config::default' crates --include='*.rs'; then
  echo "SPEX must keep the audited yamux 0.13 default configuration path" >&2
  exit 1
fi

echo "release documentation gate passed"
