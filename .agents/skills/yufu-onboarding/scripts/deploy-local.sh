#!/usr/bin/env bash
# Starts (or reuses) a loopback yufu deployment for local integration work.
# Idempotent: a reachable issuer is reused, an existing database is kept.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

load_versions
TARGET=$(detect_target)
SOURCE_ROOT=$(detect_source_checkout)
if [ -n "$SOURCE_ROOT" ]; then
    COMPOSE_FILE="$SOURCE_ROOT/compose.yaml"
    COMPOSE_PROJECT=""
else
    COMPOSE_FILE="$YUFU_SKILL_DIR/assets/compose.postgres.yaml"
    COMPOSE_PROJECT=yufu-onboarding
fi
PORT=${YUFU_PORT:-14000}
PG_PORT=${YUFU_PG_PORT:-55433}
DB_NAME=${YUFU_DB_NAME:-yufu_onboarding}
ADMIN_USER=${YUFU_ADMIN_USERNAME:-onboard-admin}
ADMIN_PASSWORD=${YUFU_ADMIN_PASSWORD:-onboard-admin-password}
ISSUER=${YUFU_ISSUER:-http://127.0.0.1:$PORT}
CONFIG="$YUFU_WORKDIR/yufu.toml"
LOG="$YUFU_WORKDIR/yufu.log"
KEY="$YUFU_WORKDIR/oidc_rsa_private.pem"
DB_URL="postgres://postgres:postgres@127.0.0.1:$PG_PORT/$DB_NAME"

state_init

issuer_matches() {
    local body
    body=$(curl -fsS -m 3 "$ISSUER/.well-known/openid-configuration" 2>/dev/null) || return 1
    printf '%s' "$body" | python3 -c '
import json
import sys

expected = sys.argv[1]
try:
    print("1" if json.load(sys.stdin).get("issuer") == expected else "0")
except Exception:
    print("0")
' "$ISSUER" | grep -q 1
}

remember() {
    state_set YUFU_WORKDIR "$YUFU_WORKDIR"
    state_set YUFU_ISSUER "$ISSUER"
    state_set YUFU_PORT "$PORT"
    state_set YUFU_PG_PORT "$PG_PORT"
    state_set YUFU_DB_NAME "$DB_NAME"
    state_set YUFU_CONFIG "$CONFIG"
    state_set YUFU_ADMIN_USER "$ADMIN_USER"
    state_set YUFU_ADMIN_PASSWORD "$ADMIN_PASSWORD"
    state_set YUFU_SERVER_LOG "$LOG"
    state_set YUFU_CLIENT_SECRET_FILE "$YUFU_WORKDIR/client_secret"
    state_set YUFU_VERSION "$YUFU_VERSION"
    state_set YUFU_TARGET "$TARGET"
    if [ -n "${YUFU_BINARY:-}" ]; then
        state_set YUFU_BINARY "$YUFU_BINARY"
        state_set YUFU_BINARY_SOURCE "$YUFU_BINARY_SOURCE"
    fi
}

probe_recorded_binary() {
    local recorded cached
    recorded=$(state_get YUFU_BINARY 2>/dev/null || true)
    if [ -n "$recorded" ] && [ -x "$recorded" ]; then
        YUFU_BINARY="$recorded"
        YUFU_BINARY_SOURCE=$(state_get YUFU_BINARY_SOURCE 2>/dev/null || echo unknown)
        return 0
    fi
    if [ -n "${YUFU_BIN:-}" ] && [ -x "$YUFU_BIN" ]; then
        YUFU_BINARY="$YUFU_BIN"
        YUFU_BINARY_SOURCE=env
        return 0
    fi
    if [ -n "$SOURCE_ROOT" ] && [ -x "$SOURCE_ROOT/target/debug/yufu" ]; then
        YUFU_BINARY="$SOURCE_ROOT/target/debug/yufu"
        YUFU_BINARY_SOURCE=checkout
        return 0
    fi
    cached=$(yufu_cached_bin "$YUFU_VERSION" "$TARGET")
    if [ -n "$cached" ]; then
        YUFU_BINARY="$cached"
        YUFU_BINARY_SOURCE=cache
        return 0
    fi
    return 1
}

if issuer_matches; then
    log_ok "reusing yufu already listening on $ISSUER"
    probe_recorded_binary || true
    remember
    if [ -z "${YUFU_BINARY:-}" ]; then
        state_set YUFU_BINARY_SOURCE unknown
    fi
    exit 0
fi

if command -v lsof >/dev/null 2>&1 && lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
    die "port $PORT is in use by another process; set YUFU_PORT or stop it"
fi

if docker compose version >/dev/null 2>&1; then
    COMPOSE=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE=(docker-compose)
else
    die "docker compose is required for the local database"
fi

need_cmd curl
need_cmd python3

ensure_postgres "$COMPOSE_FILE" "$COMPOSE_PROJECT" "$PG_PORT"

if ! docker exec postgres_yufu \
    psql -U postgres -tAc "SELECT 1 FROM pg_database WHERE datname = '$DB_NAME'" | grep -q 1; then
    log_info "creating database $DB_NAME"
    docker exec postgres_yufu createdb -U postgres "$DB_NAME"
fi

if [ ! -f "$KEY" ]; then
    need_cmd openssl
    log_info "generating a loopback signing key"
    openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$KEY" 2>/dev/null
    chmod 600 "$KEY"
fi

log_info "rendering $CONFIG"
python3 - "$YUFU_SKILL_DIR/assets/yufu.local.toml.tmpl" "$CONFIG" \
    "PORT=$PORT" "ISSUER=$ISSUER" "ADMIN_USER=$ADMIN_USER" \
    "ADMIN_PASSWORD=$ADMIN_PASSWORD" "WORKDIR=$YUFU_WORKDIR" "DATABASE_URL=$DB_URL" <<'PY'
import sys

template = open(sys.argv[1], encoding="utf-8").read()
out = sys.argv[2]
for pair in sys.argv[3:]:
    key, value = pair.split("=", 1)
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    template = template.replace("{{" + key + "}}", escaped)
open(out, "w", encoding="utf-8").write(template)
PY
chmod 600 "$CONFIG"

resolve_yufu_bin >/dev/null
BIN="$YUFU_BINARY"
log_info "using yufu binary ($YUFU_BINARY_SOURCE): $BIN"
[ -x "$BIN" ] || die "yufu binary is not executable: $BIN"

log_info "starting yufu on $ISSUER (log: $LOG)"
nohup "$BIN" --config "$CONFIG" >"$LOG" 2>&1 &
pid=$!

for _ in $(seq 1 60); do
    if issuer_matches; then
        remember
        state_set YUFU_SERVER_PID "$pid"
        log_ok "yufu ready: issuer=$ISSUER pid=$pid"
        exit 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
        log_fail "yufu exited during startup; last log lines:"
        tail -n 40 "$LOG" >&2 || true
        exit 1
    fi
    sleep 1
done

log_fail "yufu did not become ready on $ISSUER; last log lines:"
tail -n 40 "$LOG" >&2 || true
kill "$pid" 2>/dev/null || true
exit 1