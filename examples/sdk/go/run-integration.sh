#!/usr/bin/env bash
#
# Runs the Go Runtime SDK live integration test against a real cobotusr.
#
# Two modes:
#
#   local harness (default, COBOTUSR_ISSUER unset)
#     1. starts (or reuses) the Postgres container from compose.yaml
#     2. creates the integration database if it does not exist
#     3. writes a loopback-only cobotusr config under /tmp/cobotusr-sdk-it
#     4. starts cobotusr (or reuses one already listening on the same issuer)
#     5. runs `go test -tags=integration -run TestLive -v` with the required env
#     6. stops the server it started (the database container is left running)
#
#   existing deployment (COBOTUSR_ISSUER set)
#     Skips Postgres and server startup and runs the test against that issuer.
#     COBOTUSR_ADMIN_USERNAME and COBOTUSR_ADMIN_PASSWORD are then required.
#
# Overridable environment:
#   COBOTUSR_ISSUER           test an existing deployment instead of the local harness
#   COBOTUSR_IT_PORT          local harness port    (default 14000)
#   COBOTUSR_IT_DIR           work dir              (default /tmp/cobotusr-sdk-it)
#   COBOTUSR_IT_DB            database name         (default cobotusr_sdk_it)
#   POSTGRES_USR_PORT         host port for Postgres (default 55433)
#   COBOTUSR_ADMIN_USERNAME   bootstrap admin       (default sdk-admin)
#   COBOTUSR_ADMIN_PASSWORD   bootstrap password    (default sdk-admin-password)
#   COBOTUSR_IT_SKIP_BUILD=1  reuse target/debug/cobotusr without rebuilding
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$SCRIPT_DIR/../../.." && pwd)

PORT=${COBOTUSR_IT_PORT:-14000}
WORKDIR=${COBOTUSR_IT_DIR:-/tmp/cobotusr-sdk-it}
DB_NAME=${COBOTUSR_IT_DB:-cobotusr_sdk_it}
PG_PORT=${POSTGRES_USR_PORT:-55433}
ADMIN_USER=${COBOTUSR_ADMIN_USERNAME:-sdk-admin}
ADMIN_PASS=${COBOTUSR_ADMIN_PASSWORD:-sdk-admin-password}
CONFIG="$WORKDIR/cobotusr.toml"
LOG="$WORKDIR/server.log"

run_live_test() {
  cd "$SCRIPT_DIR"
  COBOTUSR_ISSUER="$ISSUER" \
  COBOTUSR_ADMIN_USERNAME="$ADMIN_USER" \
  COBOTUSR_ADMIN_PASSWORD="$ADMIN_PASS" \
    go test -tags=integration -run TestLive -v -count=1 -timeout 300s ./...
}

if [[ -n "${COBOTUSR_ISSUER:-}" ]]; then
  ISSUER=${COBOTUSR_ISSUER%/}
  echo "==> using externally configured issuer: $ISSUER"
  echo "    no local Postgres or cobotusr will be started"
  if [[ -z "${COBOTUSR_ADMIN_USERNAME:-}" || -z "${COBOTUSR_ADMIN_PASSWORD:-}" ]]; then
    echo "error: COBOTUSR_ADMIN_USERNAME and COBOTUSR_ADMIN_PASSWORD are required when COBOTUSR_ISSUER is set" >&2
    exit 1
  fi
  run_live_test
  exit $?
fi

ISSUER="http://127.0.0.1:${PORT}"

if command -v docker-compose >/dev/null 2>&1; then
  COMPOSE=(docker-compose)
elif docker compose version >/dev/null 2>&1; then
  COMPOSE=(docker compose)
else
  echo "error: neither docker-compose nor 'docker compose' is available" >&2
  exit 1
fi

mkdir -p "$WORKDIR"

echo "==> local harness issuer: $ISSUER"
echo "    (set COBOTUSR_ISSUER to test an existing deployment instead)"

echo "==> starting Postgres (host port $PG_PORT)"
# Force recreate so the published host port always matches $PG_PORT. The data
# volume is preserved; only the container is replaced.
POSTGRES_USR_PORT="$PG_PORT" "${COMPOSE[@]}" -f "$ROOT/compose.yaml" up -d --force-recreate postgres_cobotusr >/dev/null

echo "==> waiting for Postgres"
for _ in $(seq 1 60); do
  if "${COMPOSE[@]}" -f "$ROOT/compose.yaml" exec -T postgres_cobotusr \
      pg_isready -U postgres >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
if ! "${COMPOSE[@]}" -f "$ROOT/compose.yaml" exec -T postgres_cobotusr \
    pg_isready -U postgres >/dev/null 2>&1; then
  echo "error: Postgres did not become ready" >&2
  exit 1
fi

if ! "${COMPOSE[@]}" -f "$ROOT/compose.yaml" exec -T postgres_cobotusr \
    psql -U postgres -tAc "SELECT 1 FROM pg_database WHERE datname = '$DB_NAME'" | grep -q 1; then
  echo "==> creating database $DB_NAME"
  "${COMPOSE[@]}" -f "$ROOT/compose.yaml" exec -T postgres_cobotusr \
    createdb -U postgres "$DB_NAME"
fi

echo "==> writing $CONFIG"
cat >"$CONFIG" <<EOF
[http]
listen = "127.0.0.1:${PORT}"
api_prefix = "/api/v1"

[auth]
allow_insecure_loopback_cookies = true
exp_seconds = 3600
refresh_exp_seconds = 86400
origins = ["${ISSUER}"]

[auth.super_admin]
username = "${ADMIN_USER}"
password = "${ADMIN_PASS}"

[oidc]
issuer = "${ISSUER}"
rsa_private_key_path = "${WORKDIR}/oidc.pem"
signing_algorithm = "RS256"

[database]
url = "postgres://postgres:postgres@127.0.0.1:${PG_PORT}/${DB_NAME}"

[security_clock]
servers = ["time.cloudflare.com:123", "time.google.com:123", "ntp.aliyun.com:123"]
min_agreeing_sources = 2
poll_interval_seconds = 20
sample_ttl_seconds = 60
timeout_millis = 2000

[log]
level = "info"
EOF

SERVER_PID=""
cleanup() {
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "==> stopping cobotusr (pid $SERVER_PID)"
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

if curl -fsS -m 3 "$ISSUER/.well-known/openid-configuration" 2>/dev/null | grep -q "\"issuer\":\"${ISSUER}\""; then
  echo "==> reusing cobotusr already listening on $ISSUER"
else
  BIN="$ROOT/target/debug/cobotusr"
  if [[ "${COBOTUSR_IT_SKIP_BUILD:-0}" != "1" ]]; then
    echo "==> building cobotusr"
    (cd "$ROOT" && cargo build --bin cobotusr)
  fi
  if [[ ! -x "$BIN" ]]; then
    echo "error: $BIN is missing; run without COBOTUSR_IT_SKIP_BUILD=1" >&2
    exit 1
  fi
  echo "==> starting cobotusr on $ISSUER (log: $LOG)"
  "$BIN" --config "$CONFIG" >"$LOG" 2>&1 &
  SERVER_PID=$!
  for _ in $(seq 1 60); do
    if curl -fsS -m 2 "$ISSUER/.well-known/openid-configuration" >/dev/null 2>&1; then
      break
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
      echo "error: cobotusr exited during startup; last log lines:" >&2
      tail -n 40 "$LOG" >&2 || true
      exit 1
    fi
    sleep 1
  done
  if ! curl -fsS -m 2 "$ISSUER/.well-known/openid-configuration" >/dev/null 2>&1; then
    echo "error: cobotusr did not become ready; last log lines:" >&2
    tail -n 40 "$LOG" >&2 || true
    exit 1
  fi
fi

echo "==> running live integration test"
run_live_test
