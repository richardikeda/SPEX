#!/usr/bin/env bash
set -euo pipefail

# This checker rejects expired rows in the normative advisory-exception table.
# The optional date argument makes the parser deterministic in negative tests.
exceptions_file="${1:-docs/security-advisory-exceptions.md}"
today_utc="${2:-$(date -u +%F)}"

expired_advisories="$(
  awk -F'|' -v today="$today_utc" '
    BEGIN {
      today_compact = today
      gsub(/-/, "", today_compact)
    }
    $6 ~ /^[[:space:]]*20[0-9]{2}-[0-9]{2}-[0-9]{2}[[:space:]]*$/ {
      advisory = $2
      expiry = $6
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", advisory)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", expiry)
      expiry_compact = expiry
      gsub(/-/, "", expiry_compact)
      if ((expiry_compact + 0) < (today_compact + 0)) {
        print advisory " expired on " expiry
      }
    }
  ' "$exceptions_file"
)"

if [[ -n "$expired_advisories" ]]; then
  echo "security advisory exception approval expired:" >&2
  echo "$expired_advisories" >&2
  exit 1
fi

echo "security advisory exception expiry check passed"
