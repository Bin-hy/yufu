#!/usr/bin/env bash
# Registers a development application through the console admin API.
# Idempotent: the same project reuses a recorded client id.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

MODE=""
REDIRECT_URI=""
POST_LOGOUT_URI=""
AUDIENCE=""
SCOPES=""
APP_KEY=""

while [ $# -gt 0 ]; do
    case "$1" in
        --mode) MODE="$2"; shift 2 ;;
        --redirect-uri) REDIRECT_URI="$2"; shift 2 ;;
        --post-logout-uri) POST_LOGOUT_URI="$2"; shift 2 ;;
        --audience) AUDIENCE="$2"; shift 2 ;;
        --scopes) SCOPES="$2"; shift 2 ;;
        --app-key) APP_KEY="$2"; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done

case "$MODE" in
    web) CLIENT_KIND=server_web ;;
    api) CLIENT_KIND=browser_spa ;;
    *) die "--mode web|api is required" ;;
esac

ISSUER=$(state_get YUFU_ISSUER 2>/dev/null || true)
[ -n "$ISSUER" ] || die "no issuer recorded; run scripts/deploy-local.sh first"
ADMIN_USER=$(state_get YUFU_ADMIN_USER 2>/dev/null || true)
ADMIN_PASSWORD=$(state_get YUFU_ADMIN_PASSWORD 2>/dev/null || true)
[ -n "$ADMIN_USER" ] && [ -n "$ADMIN_PASSWORD" ] || die "no bootstrap credentials recorded in state"

PROJECT_DIR=${YUFU_PROJECT_DIR:-$(pwd)}
REDIRECT_URI=${REDIRECT_URI:-http://127.0.0.1:8080/callback}
POST_LOGOUT_URI=${POST_LOGOUT_URI:-http://127.0.0.1:8080/}
AUDIENCE=${AUDIENCE:-urn:app:development}
SCOPES=${SCOPES:-openid profile email}
APP_KEY=${APP_KEY:-$(state_get YUFU_APP_KEY 2>/dev/null || true)}
if [ -z "$APP_KEY" ]; then
    digest=$(printf '%s' "$PROJECT_DIR" | (shasum -a 256 2>/dev/null || sha256sum) | cut -c1-8)
    APP_KEY="onboard-$digest"
fi

# yufu validates post_logout_redirect_uri against redirect_uris, so both
# addresses must be registered.
REDIRECT_URIS=$(python3 - "$REDIRECT_URI" "$POST_LOGOUT_URI" <<'PY'
import json
import sys

uris = []
for value in sys.argv[1:3]:
    if value not in uris:
        uris.append(value)
print(json.dumps(uris))
PY
)
if [ "$CLIENT_KIND" = "browser_spa" ]; then
    BROWSER_ORIGINS=$(python3 - "$REDIRECT_URI" "$POST_LOGOUT_URI" <<'PY'
import json
import sys
import urllib.parse

origins = []
for value in sys.argv[1:3]:
    parts = urllib.parse.urlparse(value)
    origin = f"{parts.scheme}://{parts.netloc}"
    if origin not in origins:
        origins.append(origin)
print(json.dumps(origins))
PY
)
    PRODUCTION_BROWSER_ORIGINS='["https://onboarding.invalid"]'
else
    BROWSER_ORIGINS='[]'
    PRODUCTION_BROWSER_ORIGINS='[]'
fi
SECRET_FILE=$(state_get YUFU_CLIENT_SECRET_FILE 2>/dev/null || true)
SECRET_FILE=${SECRET_FILE:-$YUFU_WORKDIR/client_secret}

existing=$(state_get YUFU_CLIENT_ID 2>/dev/null || true)
if [ -n "$existing" ]; then
    if [ "$MODE" = "web" ] && [ ! -s "$SECRET_FILE" ]; then
        die "client $existing is recorded but $SECRET_FILE is missing; secrets are only returned once. Reset the client secret in the console or register with --app-key onboard-$APP_KEY-2"
    fi
    log_info "application already recorded (client_id=$existing); verifying"
    exec bash "$SCRIPT_DIR/verify-app.sh"
fi

state_set YUFU_MODE "$MODE"
state_set YUFU_PROJECT_DIR "$PROJECT_DIR"
state_set YUFU_APP_KEY "$APP_KEY"
state_set YUFU_AUDIENCE "$AUDIENCE"
state_set YUFU_REDIRECT_URI "$REDIRECT_URI"
state_set YUFU_POST_LOGOUT_URI "$POST_LOGOUT_URI"
state_set YUFU_SCOPES "$SCOPES"
state_set YUFU_REGISTERED_REDIRECT_URI "$REDIRECT_URI"
state_set YUFU_REGISTERED_POST_LOGOUT_URI "$POST_LOGOUT_URI"
state_set YUFU_REGISTERED_AUDIENCE "$AUDIENCE"
state_set YUFU_REGISTERED_SCOPES "$SCOPES"
state_set YUFU_CLIENT_SECRET_FILE "$SECRET_FILE"

JAR="$YUFU_WORKDIR/cookies.txt"
umask 077
: >"$JAR"

login=$(browser_login "$ISSUER" "$ADMIN_USER" "$ADMIN_PASSWORD" "$JAR")
csrf=$(printf '%s' "$login" | json_get csrf_token)
owner=$(printf '%s' "$login" | json_get user_id)

payload_tmp=$(mktemp "$YUFU_WORKDIR/payload.XXXXXX")
python3 - "$YUFU_SKILL_DIR/assets/registration.json.tmpl" "$payload_tmp" \
    "APP_KEY=$APP_KEY" "APP_NAME=onboarding $APP_KEY" "CLIENT_KIND=$CLIENT_KIND" \
    "REDIRECT_URI=$REDIRECT_URI" "POST_LOGOUT_URI=$POST_LOGOUT_URI" "REDIRECT_URIS=$REDIRECT_URIS" \
    "BROWSER_ORIGINS=$BROWSER_ORIGINS" "PRODUCTION_BROWSER_ORIGINS=$PRODUCTION_BROWSER_ORIGINS" \
    "AUDIENCE=$AUDIENCE" "SCOPES=$SCOPES" \
    "OWNER_USER_ID=$owner" <<'PY'
import json
import sys

template = open(sys.argv[1], encoding="utf-8").read()
raw_keys = {"OWNER_USER_ID", "BROWSER_ORIGINS", "PRODUCTION_BROWSER_ORIGINS", "REDIRECT_URIS"}
for pair in sys.argv[3:]:
    key, value = pair.split("=", 1)
    template = template.replace("{{" + key + "}}", value if key in raw_keys else json.dumps(value)[1:-1])
json.loads(template)
open(sys.argv[2], "w", encoding="utf-8").write(template)
PY

response_file=$(mktemp "$YUFU_WORKDIR/registration.XXXXXX")
status=$(curl -sS -m 20 -c "$JAR" -b "$JAR" -X POST \
    -H "Origin: $ISSUER" -H "Content-Type: application/json" \
    -H "x-csrf-token: $csrf" -H "idempotency-key: $APP_KEY" \
    --data-binary "@$payload_tmp" \
    -o "$response_file" -w '%{http_code}' "$ISSUER/api/v1/admin/apps" 2>/dev/null || true)

if [ "$status" != "200" ]; then
    log_fail "registration returned HTTP $status"
    head -c 500 "$response_file" >&2 || true
    printf '\n' >&2
    rm -f "$payload_tmp" "$response_file"
    log_info "if the app_key already exists, reuse --app-key or remove the stale application in the console"
    exit 1
fi

metadata=$(python3 - "$response_file" "$SECRET_FILE" <<'PY'
import json
import os
import sys

payload = json.load(open(sys.argv[1], encoding="utf-8"))
development = next(
    (env for env in payload.get("environments", []) if env.get("kind") == "development"),
    None,
)
if development is None:
    print(json.dumps({"error": "no development environment in response"}))
    sys.exit(0)
secret = next(
    (item.get("client_secret") for item in payload.get("newly_issued_secrets", [])
     if item.get("environment_id") == development.get("environment_id")),
    None,
)
if secret:
    path = sys.argv[2]
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(secret + "\n")
    os.chmod(path, 0o600)
print(json.dumps({
    "client_id": development.get("client_id", ""),
    "environment_id": development.get("environment_id", ""),
    "application_id": payload.get("application_id", ""),
    "has_secret": bool(secret),
}))
PY
)
rm -f "$payload_tmp" "$response_file"

client_id=$(printf '%s' "$metadata" | json_get client_id)
[ -n "$client_id" ] || die "registration response did not contain a development client_id"

has_secret=$(printf '%s' "$metadata" | json_get has_secret)
if [ "$MODE" = "web" ] && [ "$has_secret" != "True" ]; then
    die "client $client_id was registered without a new secret; reset the secret in the console and rerun"
fi

state_set YUFU_CLIENT_ID "$client_id"
state_set YUFU_ENVIRONMENT_ID "$(printf '%s' "$metadata" | json_get environment_id)"
state_set YUFU_APP_ID "$(printf '%s' "$metadata" | json_get application_id)"

log_ok "registered application client_id=$client_id app_key=$APP_KEY"
if [ "$has_secret" = "True" ]; then
    log_ok "client secret written to $SECRET_FILE (mode 600)"
fi
exec bash "$SCRIPT_DIR/verify-app.sh"