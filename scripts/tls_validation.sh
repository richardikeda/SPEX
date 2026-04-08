#!/usr/bin/env bash
# tls_validation.sh — SPEX bridge TLS deployment validation script
#
# Runs the mandatory pre-release TLS validation checklist defined in
# docs/bridge-tls-deployment.md.
#
# Usage:
#   ./scripts/tls_validation.sh <BRIDGE_HOST> [OPTIONS]
#
# Examples:
#   ./scripts/tls_validation.sh bridge.example.com
#   ./scripts/tls_validation.sh bridge.example.com --plain-port 3000
#
# Environment variables:
#   BRIDGE_HOST    Bridge hostname (overrides positional argument)
#   BRIDGE_PORT    HTTPS port (default: 443)
#   PLAIN_PORT     Plain HTTP port to test for rejection (default: 3000)
#   TIMEOUT        Per-check timeout in seconds (default: 10)
#   SKIP_PLAIN     Set to "1" to skip the plain-HTTP rejection check (use in CI)
#
# Exit codes:
#   0  All checks passed — deployment is TLS-ready.
#   1  One or more checks failed.

set -euo pipefail

# ── Configuration ────────────────────────────────────────────────────────────

BRIDGE_HOST="${BRIDGE_HOST:-${1:-}}"
BRIDGE_PORT="${BRIDGE_PORT:-443}"
PLAIN_PORT="${PLAIN_PORT:-3000}"
TIMEOUT="${TIMEOUT:-10}"
SKIP_PLAIN="${SKIP_PLAIN:-0}"

if [[ -z "${BRIDGE_HOST}" ]]; then
    echo "ERROR: BRIDGE_HOST is required." >&2
    echo "Usage: $0 <host> [--plain-port <port>]" >&2
    exit 1
fi

shift || true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --plain-port) PLAIN_PORT="${2}"; shift 2 ;;
        --port)       BRIDGE_PORT="${2}"; shift 2 ;;
        --timeout)    TIMEOUT="${2}"; shift 2 ;;
        --skip-plain) SKIP_PLAIN="1"; shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

PASS=0
FAIL=0
EVIDENCE_FILE="${EVIDENCE_FILE:-tls-validation-evidence.txt}"

# ── Helpers ───────────────────────────────────────────────────────────────────

log_result() {
    local status="$1"
    local check="$2"
    local detail="${3:-}"
    if [[ "${status}" == "PASS" ]]; then
        echo "[PASS] ${check}"
        PASS=$((PASS + 1))
    else
        echo "[FAIL] ${check}"
        FAIL=$((FAIL + 1))
    fi
    if [[ -n "${detail}" ]]; then
        echo "       ${detail}"
    fi
}

require_tool() {
    if ! command -v "$1" &>/dev/null; then
        echo "ERROR: Required tool not found: $1" >&2
        exit 1
    fi
}

# ── Pre-flight ────────────────────────────────────────────────────────────────

require_tool openssl
require_tool curl

echo "====================================================================="
echo " SPEX Bridge TLS Validation Checklist"
echo " Host  : ${BRIDGE_HOST}:${BRIDGE_PORT}"
echo " Date  : $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "====================================================================="
echo ""

# Redirect all output to evidence file AND stdout.
exec > >(tee -a "${EVIDENCE_FILE}") 2>&1

# ── Check 1: Certificate validity ────────────────────────────────────────────

echo "--- Check 1: Certificate validity ---"
NOT_AFTER=$(openssl s_client \
    -connect "${BRIDGE_HOST}:${BRIDGE_PORT}" \
    -servername "${BRIDGE_HOST}" \
    -timeout "${TIMEOUT}" \
    </dev/null 2>/dev/null \
    | openssl x509 -noout -dates 2>/dev/null \
    | grep notAfter | cut -d= -f2 || true)

if [[ -n "${NOT_AFTER}" ]]; then
    EXPIRY_EPOCH=$(date -d "${NOT_AFTER}" +%s 2>/dev/null || date -j -f "%b %e %T %Y %Z" "${NOT_AFTER}" +%s 2>/dev/null || echo 0)
    NOW_EPOCH=$(date +%s)
    if [[ "${EXPIRY_EPOCH}" -gt "${NOW_EPOCH}" ]]; then
        log_result PASS "Certificate is valid" "Expires: ${NOT_AFTER}"
    else
        log_result FAIL "Certificate has expired" "Expired: ${NOT_AFTER}"
    fi
else
    log_result FAIL "Could not retrieve certificate" "Check that the bridge is reachable on port ${BRIDGE_PORT}"
fi

# ── Check 2: TLS protocol enforcement ────────────────────────────────────────

echo ""
echo "--- Check 2: TLS protocol enforcement ---"

# TLS 1.0 must be rejected.
TLS10_OUTPUT=$(openssl s_client \
    -connect "${BRIDGE_HOST}:${BRIDGE_PORT}" \
    -tls1 \
    -timeout "${TIMEOUT}" \
    </dev/null 2>&1 || true)
if echo "${TLS10_OUTPUT}" | grep -qi "handshake failure\|alert\|no protocols available\|wrong version number"; then
    log_result PASS "TLS 1.0 rejected" ""
else
    log_result FAIL "TLS 1.0 not rejected (server may accept insecure protocol)" ""
fi

# TLS 1.1 must be rejected.
TLS11_OUTPUT=$(openssl s_client \
    -connect "${BRIDGE_HOST}:${BRIDGE_PORT}" \
    -tls1_1 \
    -timeout "${TIMEOUT}" \
    </dev/null 2>&1 || true)
if echo "${TLS11_OUTPUT}" | grep -qi "handshake failure\|alert\|no protocols available\|wrong version number"; then
    log_result PASS "TLS 1.1 rejected" ""
else
    log_result FAIL "TLS 1.1 not rejected (server may accept insecure protocol)" ""
fi

# TLS 1.2 must be accepted.
TLS12_CIPHER=$(openssl s_client \
    -connect "${BRIDGE_HOST}:${BRIDGE_PORT}" \
    -tls1_2 \
    -timeout "${TIMEOUT}" \
    </dev/null 2>/dev/null \
    | grep "Cipher is" || true)
if [[ -n "${TLS12_CIPHER}" ]]; then
    log_result PASS "TLS 1.2 accepted" "${TLS12_CIPHER}"
else
    log_result FAIL "TLS 1.2 not accepted" "Server must accept at minimum TLS 1.2"
fi

# ── Check 3: HTTP redirect ────────────────────────────────────────────────────

echo ""
echo "--- Check 3: Plain HTTP redirected to HTTPS ---"

HTTP_STATUS=$(curl \
    --silent \
    --max-time "${TIMEOUT}" \
    --output /dev/null \
    --write-out "%{http_code}" \
    "http://${BRIDGE_HOST}/slot/healthcheck" \
    2>/dev/null || echo "000")

if [[ "${HTTP_STATUS}" == "301" || "${HTTP_STATUS}" == "302" || "${HTTP_STATUS}" == "307" || "${HTTP_STATUS}" == "308" ]]; then
    log_result PASS "HTTP redirects to HTTPS (${HTTP_STATUS})" ""
elif [[ "${HTTP_STATUS}" == "000" ]]; then
    log_result PASS "HTTP connection refused (no plain-HTTP service exposed)" ""
else
    log_result FAIL "HTTP not redirected (status ${HTTP_STATUS}) — plain HTTP may be served" ""
fi

# ── Check 4: Certificate chain trust ─────────────────────────────────────────

echo ""
echo "--- Check 4: Certificate chain trusted by system CA store ---"

CURL_STATUS=$(curl \
    --silent \
    --max-time "${TIMEOUT}" \
    --output /dev/null \
    --write-out "%{http_code}" \
    "https://${BRIDGE_HOST}:${BRIDGE_PORT}/slot/nonexistent" \
    2>/dev/null || echo "000")

if [[ "${CURL_STATUS}" == "404" || "${CURL_STATUS}" == "200" || "${CURL_STATUS}" == "401" || "${CURL_STATUS}" == "400" ]]; then
    log_result PASS "Certificate chain trusted (HTTP ${CURL_STATUS} received)" ""
elif [[ "${CURL_STATUS}" == "000" ]]; then
    log_result FAIL "TLS handshake failed or connection refused" "Check certificate chain validity"
else
    log_result FAIL "Unexpected response (status ${CURL_STATUS})" ""
fi

# ── Check 5: Plain HTTP port inaccessible externally ─────────────────────────

echo ""
echo "--- Check 5: Bridge not reachable on plain HTTP port ${PLAIN_PORT} externally ---"

if [[ "${SKIP_PLAIN}" == "1" ]]; then
    echo "[SKIP] Plain-HTTP external rejection check skipped (SKIP_PLAIN=1)"
else
    PLAIN_RESPONSE=$(curl \
        --silent \
        --max-time "${TIMEOUT}" \
        --output /dev/null \
        --write-out "%{http_code}" \
        "http://${BRIDGE_HOST}:${PLAIN_PORT}/" \
        2>/dev/null || echo "000")

    if [[ "${PLAIN_RESPONSE}" == "000" ]]; then
        log_result PASS "Bridge plain HTTP port ${PLAIN_PORT} is not reachable externally" ""
    else
        log_result FAIL "Bridge plain HTTP port ${PLAIN_PORT} is reachable externally (status ${PLAIN_RESPONSE})" \
            "Bind bridge to loopback only: SPEX_BRIDGE_ADDR=127.0.0.1:${PLAIN_PORT}"
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "====================================================================="
echo " Results: ${PASS} passed, ${FAIL} failed"
echo " Evidence written to: ${EVIDENCE_FILE}"
echo "====================================================================="

if [[ "${FAIL}" -gt 0 ]]; then
    echo "DEPLOYMENT IS NOT TLS-READY. Fix the failing checks before going public."
    exit 1
else
    echo "ALL CHECKS PASSED. This deployment meets the SPEX TLS requirement."
    echo "Attach ${EVIDENCE_FILE} as release evidence for v$(cat VERSION.md | cut -d= -f2 2>/dev/null || echo 'unknown')."
    exit 0
fi
