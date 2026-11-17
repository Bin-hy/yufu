#!/usr/bin/env bash
# One non-interactive entry: preflight -> deploy -> verify issuer -> register.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
WORKDIR_BEFORE=${YUFU_WORKDIR:-}
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

MODE=""
REDIRECT_URI=""
POST_LOGOUT_URI=""
AUDIENCE=""
SCOPES=""
APP_KEY=""
PROJECT_DIR=""

usage() {
    cat >&2 <<'EOF'
usage: onboard.sh --mode web|api [options]

  --mode web|api          web = SSO backend, api = bearer-token resource server
  --redirect-uri URI      web: OAuth redirect (default: first free loopback port from 8080)
  --post-logout-uri URI   web: post-logout redirect (default: same origin as the redirect)
  --audience AUD          token audience (default urn:app:development)
  --scopes SCOPES         space-separated scopes (default "openid profile email")
  --app-key KEY           idempotency key; defaults to a hash of the project path
  --project-dir DIR       project that owns the app key (default: current directory)
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --mode) MODE="$2"; shift 2 ;;
        --redirect-uri) REDIRECT_URI="$2"; shift 2 ;;
        --post-logout-uri) POST_LOGOUT_URI="$2"; shift 2 ;;
        --audience) AUDIENCE="$2"; shift 2 ;;
        --scopes) SCOPES="$2"; shift 2 ;;
        --app-key) APP_KEY="$2"; shift 2 ;;
        --project-dir) PROJECT_DIR="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) usage; die "unknown argument: $1" ;;
    esac
done

case "$MODE" in
    web|api) ;;
    *) usage; die "--mode web|api is required" ;;
esac

PROJECT_DIR=${PROJECT_DIR:-$(pwd)}
[ -d "$PROJECT_DIR" ] || die "project directory does not exist: $PROJECT_DIR"
PROJECT_DIR=$(cd "$PROJECT_DIR" && pwd)

if [ -z "$WORKDIR_BEFORE" ]; then
    YUFU_WORKDIR="$PROJECT_DIR/.yufu-onboarding"
    STATE_FILE="$YUFU_WORKDIR/state.env"
fi
export YUFU_WORKDIR
export YUFU_PROJECT_DIR="$PROJECT_DIR"

run_stage() {
    local name="$1" code
    shift
    log_info "stage: $name"
    if "$@"; then
        return 0
    else
        code=$?
    fi
    log_fail "stage: $name failed (exit $code)"
    exit "$code"
}

run_stage preflight bash "$SCRIPT_DIR/preflight.sh"
run_stage deploy bash "$SCRIPT_DIR/deploy-local.sh"
run_stage verify-issuer bash "$SCRIPT_DIR/verify-issuer.sh"

register_args=(--mode "$MODE")
if [ -n "$REDIRECT_URI" ]; then register_args+=(--redirect-uri "$REDIRECT_URI"); fi
if [ -n "$POST_LOGOUT_URI" ]; then register_args+=(--post-logout-uri "$POST_LOGOUT_URI"); fi
if [ -n "$AUDIENCE" ]; then register_args+=(--audience "$AUDIENCE"); fi
if [ -n "$SCOPES" ]; then register_args+=(--scopes "$SCOPES"); fi
if [ -n "$APP_KEY" ]; then register_args+=(--app-key "$APP_KEY"); fi
run_stage register bash "$SCRIPT_DIR/register-app.sh" "${register_args[@]}"

ISSUER=$(state_get YUFU_ISSUER 2>/dev/null || true)
CLIENT_ID=$(state_get YUFU_CLIENT_ID 2>/dev/null || true)
SECRET_FILE=$(state_get YUFU_CLIENT_SECRET_FILE 2>/dev/null || true)
APP_PORT=$(state_get YUFU_APP_PORT 2>/dev/null || true)
if [ "$MODE" = "web" ]; then
    NEXT="references/go-web.md (SSO login; complete one browser login yourself)"
else
    NEXT="references/go-resource-server.md (bearer verification; go test covers 401/200)"
fi

printf '\n'
log_ok "onboarding complete"
printf '  issuer:     %s\n' "$ISSUER"
printf '  client_id:  %s\n' "$CLIENT_ID"
if [ -n "$SECRET_FILE" ] && [ -s "$SECRET_FILE" ]; then
    printf '  secret:     %s (mode 600; never print or commit it)\n' "$SECRET_FILE"
fi
if [ "$MODE" = "web" ] && [ -n "$APP_PORT" ]; then
    printf '  app_addr:   127.0.0.1:%s\n' "$APP_PORT"
fi
printf '  state:      %s\n' "$STATE_FILE"
printf '  project:    %s\n' "$PROJECT_DIR"
printf '  next:       %s\n' "$NEXT"
printf '  gitignore:  add .yufu-onboarding/ to the project .gitignore\n'
