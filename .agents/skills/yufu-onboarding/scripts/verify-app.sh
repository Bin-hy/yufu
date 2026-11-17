#!/usr/bin/env bash
# Verifies that the recorded client id still matches the parameters in state,
# then exercises the authorization endpoint with an authenticated session.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

ISSUER=$(state_get YUFU_ISSUER 2>/dev/null || true)
CLIENT_ID=$(state_get YUFU_CLIENT_ID 2>/dev/null || true)
[ -n "$ISSUER" ] && [ -n "$CLIENT_ID" ] || die "no registered client recorded; run scripts/register-app.sh first"

failed=0
check_field() {
    local name="$1" registered="$2" current="$3"
    if [ -z "$registered" ] || [ "$registered" = "$current" ]; then
        return 0
    fi
    log_fail "$name mismatch: registered '$registered', current '$current'"
    failed=1
}

registered_redirect=$(state_get YUFU_REGISTERED_REDIRECT_URI 2>/dev/null || true)
registered_post_logout=$(state_get YUFU_REGISTERED_POST_LOGOUT_URI 2>/dev/null || true)
registered_audience=$(state_get YUFU_REGISTERED_AUDIENCE 2>/dev/null || true)
registered_scopes=$(state_get YUFU_REGISTERED_SCOPES 2>/dev/null || true)

check_field redirect_uri "$registered_redirect" "${YUFU_REDIRECT_URI:-$registered_redirect}"
check_field post_logout_redirect_uri "$registered_post_logout" "${YUFU_POST_LOGOUT_URI:-$registered_post_logout}"
check_field access_audience "$registered_audience" "${YUFU_AUDIENCE:-$registered_audience}"
check_field identity_scopes "$registered_scopes" "${YUFU_SCOPES:-$registered_scopes}"
[ "$failed" = "0" ] || exit 1

REDIRECT_URI=${YUFU_REDIRECT_URI:-$registered_redirect}
POST_LOGOUT_URI=${YUFU_POST_LOGOUT_URI:-$registered_post_logout}
SCOPES=${YUFU_SCOPES:-$registered_scopes}

pkce=$(python3 - <<'PY'
import base64
import hashlib
import secrets

verifier = secrets.token_urlsafe(32)
challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
print(verifier)
print(challenge)
PY
)
code_challenge=$(printf '%s\n' "$pkce" | sed -n 2p)
state_value=$(python3 -c 'import secrets; print(secrets.token_urlsafe(16))')
nonce=$(python3 -c 'import secrets; print(secrets.token_urlsafe(16))')

authorize=$(python3 - "$ISSUER" "$CLIENT_ID" "$REDIRECT_URI" "$SCOPES" "$state_value" "$nonce" "$code_challenge" <<'PY'
import sys
import urllib.parse

issuer, client_id, redirect_uri, scopes, state, nonce, challenge = sys.argv[1:8]
query = urllib.parse.urlencode({
    "client_id": client_id,
    "response_type": "code",
    "response_mode": "query",
    "redirect_uri": redirect_uri,
    "scope": scopes,
    "state": state,
    "nonce": nonce,
    "code_challenge": challenge,
    "code_challenge_method": "S256",
})
print(f"{issuer}/oidc/authorize?{query}")
PY
)

ADMIN_USER=$(state_get YUFU_ADMIN_USER 2>/dev/null || true)
ADMIN_PASSWORD=$(state_get YUFU_ADMIN_PASSWORD 2>/dev/null || true)
JAR="$YUFU_WORKDIR/cookies.txt"
: >"$JAR"
browser_login "$ISSUER" "$ADMIN_USER" "$ADMIN_PASSWORD" "$JAR" >/dev/null

headers=$(mktemp)
body=$(mktemp)
trap 'rm -f "$headers" "$body"' EXIT

status=$(curl -sS -m 15 -b "$JAR" -c "$JAR" -H "Origin: $ISSUER" \
    -D "$headers" -o "$body" -w '%{http_code}' "$authorize" 2>/dev/null || true)

if [ "${status:-0}" -ge 400 ] 2>/dev/null; then
    log_fail "authorization endpoint returned HTTP $status"
    head -c 400 "$body" >&2 || true
    printf '\n' >&2
    log_info "check registered redirect_uri '$registered_redirect' and browser_origin for $CLIENT_ID"
    exit 1
fi

if grep -Eq '"(invalid_client|invalid_request|unauthorized_client)"' "$body" "$headers"; then
    log_fail "authorization endpoint rejected the request"
    grep -Eo '"(invalid_client|invalid_request|unauthorized_client)[^"]*"' "$body" "$headers" | head -n 3 >&2 || true
    log_info "compare the registered development config with $YUFU_WORKDIR/state.env"
    exit 1
fi

log_ok "client $CLIENT_ID accepted at the authorization endpoint (HTTP $status)"

[ -n "$POST_LOGOUT_URI" ] || die "no post_logout_redirect_uri recorded; rerun scripts/register-app.sh"
logout_url=$(python3 - "$ISSUER" "$CLIENT_ID" "$POST_LOGOUT_URI" <<'PY'
import sys
import urllib.parse

issuer, client_id, post_logout = sys.argv[1:4]
query = urllib.parse.urlencode({
    "client_id": client_id,
    "post_logout_redirect_uri": post_logout,
})
print(f"{issuer}/oidc/logout?{query}")
PY
)

status=$(curl -sS -m 15 -b "$JAR" -c "$JAR" -o "$body" -w '%{http_code}' "$logout_url" 2>/dev/null || true)
if [ "${status:-0}" -ge 400 ] 2>/dev/null; then
    log_fail "end-session endpoint returned HTTP $status for post_logout_redirect_uri '$POST_LOGOUT_URI'"
    head -c 400 "$body" >&2 || true
    printf '\n' >&2
    log_info "yufu checks this address against redirect_uris; register-app.sh adds both. Re-register with a new --app-key if the client predates that."
    exit 1
fi
log_ok "client $CLIENT_ID accepted at the end-session endpoint (HTTP $status)"