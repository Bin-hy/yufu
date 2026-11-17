#!/usr/bin/env bash
# Probes the local machine for what the yufu-onboarding flow needs.
# Exit 0: ready (warnings allowed). Exit 2: blocked.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

JSON=0
for arg in "$@"; do
    case "$arg" in
        --json) JSON=1 ;;
        *) die "unknown argument: $arg" ;;
    esac
done

load_versions
TARGET=""
TARGET=$(detect_target 2>/dev/null) || TARGET=""
SOURCE_ROOT=$(detect_source_checkout)
STATE_PORT=$(state_get YUFU_PORT 2>/dev/null || true)
STATE_PG_PORT=$(state_get YUFU_PG_PORT 2>/dev/null || true)
PORT=${YUFU_PORT:-${STATE_PORT:-14000}}
PG_PORT=${YUFU_PG_PORT:-${STATE_PG_PORT:-55433}}
ISSUER=${YUFU_ISSUER:-http://127.0.0.1:$PORT}

blockers=()
warnings=()
lines=()

record() { lines+=("$1"); }

if [ -z "$TARGET" ]; then
    blockers+=("unsupported platform $(uname -s)/$(uname -m); supported targets: $SUPPORTED_TARGETS")
    record "platform: unsupported"
    TARGET=unknown
fi

has_compose=0
if docker compose version >/dev/null 2>&1; then
    has_compose=1
    record "docker compose: $(docker compose version --short 2>/dev/null || echo present)"
elif command -v docker-compose >/dev/null 2>&1; then
    has_compose=1
    record "docker-compose: $(docker-compose --version 2>/dev/null || echo present)"
else
    blockers+=("docker compose is not available")
    record "docker compose: missing"
fi

for cmd in curl python3 openssl tar; do
    if command -v "$cmd" >/dev/null 2>&1; then
        record "$cmd: present"
    else
        blockers+=("$cmd is not installed")
        record "$cmd: missing"
    fi
done

if command -v go >/dev/null 2>&1; then
    go_version=$(go env GOVERSION 2>/dev/null || echo unknown)
    record "go: $go_version"
    if ! printf '%s' "$go_version" | grep -qE '^go1\.(2[2-9]|[3-9][0-9])'; then
        warnings+=("Go 1.22+ is required for the SDK (found $go_version)")
    fi
else
    warnings+=("go is not installed; the API path needs Go 1.22+")
    record "go: missing"
fi

binary_source=download
if [ -n "${YUFU_BIN:-}" ]; then
    binary_source=env
    if [ -x "$YUFU_BIN" ]; then
        record "binary: YUFU_BIN=$YUFU_BIN"
    else
        blockers+=("YUFU_BIN is not executable: $YUFU_BIN")
        record "binary: YUFU_BIN=$YUFU_BIN (not executable)"
    fi
elif [ -n "$SOURCE_ROOT" ] && [ -x "$SOURCE_ROOT/target/debug/yufu" ]; then
    binary_source=checkout
    record "binary: source checkout at $SOURCE_ROOT"
elif [ -n "$SOURCE_ROOT" ]; then
    binary_source=checkout
    if command -v cargo >/dev/null 2>&1; then
        record "binary: will build with cargo from $SOURCE_ROOT"
    else
        blockers+=("no built yufu in $SOURCE_ROOT and cargo is not installed")
        record "binary: missing and cargo is not installed"
    fi
else
    cached=$(yufu_cached_bin "$YUFU_VERSION" "$TARGET")
    if [ -n "$cached" ]; then
        binary_source=cache
        record "binary: cached at $cached"
    else
        record "binary: will download $YUFU_VERSION for $TARGET"
    fi
fi
record "yufu: $YUFU_VERSION ($TARGET)"

issuer_ready=0
if curl -fsS -m 2 "$ISSUER/.well-known/openid-configuration" >/dev/null 2>&1; then
    issuer_ready=1
    record "issuer: reachable at $ISSUER"
else
    record "issuer: not reachable at $ISSUER"
fi

if port_busy "$PORT"; then
    if [ "$issuer_ready" = "1" ]; then
        record "port $PORT: in use by the running issuer"
    elif [ -z "${YUFU_PORT:-}" ] && [ -z "${YUFU_ISSUER:-}" ]; then
        next_port=$(pick_port "$PORT" 20 || true)
        if [ -n "$next_port" ]; then
            warnings+=("port $PORT is busy; deploy will use $next_port")
            record "port $PORT: busy (deploy will use $next_port)"
        else
            blockers+=("port $PORT is busy and no free port was found nearby; set YUFU_PORT")
            record "port $PORT: busy"
        fi
    else
        blockers+=("port $PORT is already in use")
        record "port $PORT: busy"
    fi
else
    record "port $PORT: free"
fi

mapped=$(pg_published_port 2>/dev/null || true)
if [ -n "$mapped" ] && [ -z "${YUFU_PG_PORT:-}" ]; then
    record "port $mapped: postgres_yufu (will reuse)"
elif port_busy "$PG_PORT"; then
    if [ -n "${YUFU_PG_PORT:-}" ]; then
        blockers+=("port $PG_PORT is already in use by another process")
        record "port $PG_PORT: busy"
    else
        next_port=$(pick_port "$PG_PORT" 20 || true)
        if [ -n "$next_port" ]; then
            warnings+=("port $PG_PORT is busy; deploy will use $next_port for Postgres")
            record "port $PG_PORT: busy (deploy will use $next_port)"
        else
            blockers+=("port $PG_PORT is busy and no free port was found nearby; set YUFU_PG_PORT")
            record "port $PG_PORT: busy"
        fi
    fi
else
    record "port $PG_PORT: free"
fi

if [ "$(uname -s)" = "Darwin" ] && ! command -v lsof >/dev/null 2>&1; then
    warnings+=("lsof is missing; port checks fall back to nc")
fi

status=ready
[ ${#blockers[@]} -eq 0 ] || status=blocked

if [ "$JSON" = "1" ]; then
    python3 - "$status" "$ISSUER" "$issuer_ready" "$YUFU_VERSION" "$TARGET" "$binary_source" \
        ${blockers[@]+"${blockers[@]}"} -- \
        ${warnings[@]+"${warnings[@]}"} -- \
        ${lines[@]+"${lines[@]}"} <<'PY'
import json
import sys

args = sys.argv[1:]
status, issuer = args[0], args[1]
issuer_ready = args[2] == "1"
version, target, binary_source = args[3], args[4], args[5]
rest = args[6:]
blockers, warnings, lines = [], [], []
bucket = blockers
for item in rest:
    if item == "--":
        bucket = warnings if bucket is blockers else lines
        continue
    bucket.append(item)
print(json.dumps({
    "status": status,
    "issuer": issuer,
    "issuer_reachable": issuer_ready,
    "yufu_version": version,
    "target": target,
    "binary_source": binary_source,
    "blockers": blockers,
    "warnings": warnings,
    "checks": lines,
}, ensure_ascii=False))
PY
else
    for line in ${lines[@]+"${lines[@]}"}; do printf '  %s\n' "$line"; done
    for warning in ${warnings[@]+"${warnings[@]}"}; do log_info "warning: $warning"; done
    for blocker in ${blockers[@]+"${blockers[@]}"}; do log_fail "blocked: $blocker"; done
    if [ "$status" = "ready" ]; then
        log_ok "preflight passed"
    else
        log_fail "preflight blocked; resolve the items above"
        exit 2
    fi
fi

[ "$status" = "ready" ] || exit 2
