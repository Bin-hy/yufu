#!/usr/bin/env bash
# Verifies that a yufu issuer is reachable and internally consistent.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

ISSUER=$(state_get YUFU_ISSUER 2>/dev/null || true)
ISSUER=${ISSUER:-${YUFU_ISSUER:-http://127.0.0.1:${YUFU_PORT:-14000}}}
ISSUER=${ISSUER%/}

failed=0

discovery=$(curl -fsS -m 5 "$ISSUER/.well-known/openid-configuration" 2>/dev/null || true)
if [ -z "$discovery" ]; then
    log_fail "discovery unreachable: $ISSUER/.well-known/openid-configuration"
    log_info "next: run scripts/deploy-local.sh"
    exit 1
fi

reported=$(printf '%s' "$discovery" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("issuer",""))' 2>/dev/null || true)
if [ "$reported" = "$ISSUER" ]; then
    log_ok "discovery issuer matches: $ISSUER"
else
    log_fail "discovery issuer mismatch: got '$reported', want '$ISSUER'"
    failed=1
fi

jwks=$(curl -fsS -m 5 "$ISSUER/.well-known/jwks.json" 2>/dev/null || true)
key_count=$(printf '%s' "$jwks" | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("keys",[])))' 2>/dev/null || echo 0)
if [ "${key_count:-0}" -gt 0 ]; then
    log_ok "JWKS publishes $key_count key(s)"
else
    log_fail "JWKS is empty or unreachable: $ISSUER/.well-known/jwks.json"
    failed=1
fi

ready=$(curl -fsS -m 5 "$ISSUER/api/v1/health/ready" 2>/dev/null || true)
if [ -n "$ready" ]; then
    synchronized=$(printf '%s' "$ready" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("security_clock_synchronized"))' 2>/dev/null || echo False)
    if [ "$synchronized" = "True" ]; then
        log_ok "health ready and security clock synchronized"
    else
        log_fail "health ready but security clock is not synchronized"
        failed=1
    fi
else
    log_fail "health/ready returned no body; the service may be starting or NTP is blocked"
    failed=1
fi

[ "$failed" = "0" ] || { log_info "next: inspect \$(state_get YUFU_SERVER_LOG) or rerun deploy-local.sh"; exit 1; }
log_ok "issuer verified: $ISSUER"