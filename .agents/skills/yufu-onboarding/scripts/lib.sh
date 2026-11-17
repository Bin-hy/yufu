#!/usr/bin/env bash
# Shared helpers for the yufu-onboarding skill scripts.
# shellcheck shell=bash

set -euo pipefail

YUFU_SKILL_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
: "${YUFU_WORKDIR:=$(pwd)/.yufu-onboarding}"
STATE_FILE="$YUFU_WORKDIR/state.env"

log_info() { printf '[info] %s\n' "$*" >&2; }
log_ok() { printf '[ok] %s\n' "$*"; }
log_fail() { printf '[fail] %s\n' "$*" >&2; }

die() {
    log_fail "$*"
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

state_init() {
    mkdir -p "$YUFU_WORKDIR"
    chmod 700 "$YUFU_WORKDIR" 2>/dev/null || true
    if [ ! -f "$STATE_FILE" ]; then
        umask 077
        : >"$STATE_FILE"
    fi
    chmod 600 "$STATE_FILE" 2>/dev/null || true
}

state_get() {
    [ -f "$STATE_FILE" ] || return 1
    local line
    line=$(grep -E "^$1=" "$STATE_FILE" | tail -n 1 || true)
    [ -n "$line" ] || return 1
    printf '%s\n' "${line#*=}"
}

state_set() {
    state_init
    local tmp
    tmp=$(mktemp "${STATE_FILE}.XXXXXX")
    grep -vE "^$1=" "$STATE_FILE" >"$tmp" || true
    printf '%s=%s\n' "$1" "$2" >>"$tmp"
    mv "$tmp" "$STATE_FILE"
    chmod 600 "$STATE_FILE"
}

json_get() {
    python3 -c '
import json
import sys

path = sys.argv[1].split(".")
data = json.load(sys.stdin)
for key in path:
    if isinstance(data, list):
        data = data[int(key)]
    else:
        data = data[key]
if isinstance(data, (dict, list)):
    json.dump(data, sys.stdout)
elif data is None:
    pass
else:
    sys.stdout.write(str(data))
' "$1"
}

http_ok() {
    curl -fsS -m "${2:-3}" "$1" >/dev/null 2>&1
}

COMPOSE=()

SUPPORTED_TARGETS="aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu"

load_versions() {
    local file="$YUFU_SKILL_DIR/assets/versions.env"
    [ -f "$file" ] || die "missing version manifest: $file"
    # shellcheck disable=SC1090
    . "$file"
    [ -n "${YUFU_VERSION:-}" ] || die "YUFU_VERSION is empty in $file"
    [ -n "${YUFU_SDK_VERSION:-}" ] || die "YUFU_SDK_VERSION is empty in $file"
}

detect_target() {
    local os arch target
    if [ -n "${YUFU_TARGET:-}" ]; then
        target="$YUFU_TARGET"
    else
        os=$(uname -s)
        arch=$(uname -m)
        case "$os:$arch" in
            Darwin:arm64) target=aarch64-apple-darwin ;;
            Darwin:x86_64) target=x86_64-apple-darwin ;;
            Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
            *) die "unsupported platform $os/$arch; supported targets: $SUPPORTED_TARGETS (override with YUFU_TARGET)" ;;
        esac
    fi
    case " $SUPPORTED_TARGETS " in
        *" $target "*) ;;
        *) die "unsupported YUFU_TARGET=$target; supported targets: $SUPPORTED_TARGETS" ;;
    esac
    printf '%s\n' "$target"
}

detect_source_checkout() {
    local root=""
    if [ -n "${YUFU_ROOT:-}" ]; then
        if [ -f "$YUFU_ROOT/Cargo.toml" ] && [ -f "$YUFU_ROOT/compose.yaml" ] && [ -f "$YUFU_ROOT/src/yufu.rs" ]; then
            printf '%s\n' "$YUFU_ROOT"
            return 0
        fi
        die "YUFU_ROOT=$YUFU_ROOT is not a yufu checkout (need Cargo.toml, compose.yaml, src/yufu.rs)"
    fi
    root=$(cd "$YUFU_SKILL_DIR/../../.." 2>/dev/null && pwd || true)
    if [ -n "$root" ] && [ -f "$root/Cargo.toml" ] && [ -f "$root/compose.yaml" ] && [ -f "$root/src/yufu.rs" ]; then
        printf '%s\n' "$root"
    fi
    return 0
}

yufu_cached_bin() {
    local version="$1" target="$2" path
    path="$YUFU_WORKDIR/bin/yufu-$version-$target/yufu"
    if [ -x "$path" ]; then
        printf '%s\n' "$path"
    fi
    return 0
}

sha256_of() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    elif command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        die "need shasum or sha256sum to verify the download"
    fi
}

# resolve_yufu_bin echoes the chosen executable and also records it in the
# globals YUFU_BINARY and YUFU_BINARY_SOURCE (env, checkout, cache, download).
# Callers that need the source must call it directly, not inside $(...).
resolve_yufu_bin() {
    load_versions
    local version target root bin sha_key expected actual url tmp candidate dest
    version="$YUFU_VERSION"
    target=$(detect_target)

    if [ -n "${YUFU_BIN:-}" ]; then
        [ -x "$YUFU_BIN" ] || die "YUFU_BIN is not executable: $YUFU_BIN"
        YUFU_BINARY_SOURCE=env
        YUFU_BINARY="$YUFU_BIN"
        printf '%s\n' "$YUFU_BIN"
        return 0
    fi

    root=$(detect_source_checkout)
    if [ -n "$root" ]; then
        bin="$root/target/debug/yufu"
        if [ -x "$bin" ]; then
            YUFU_BINARY_SOURCE=checkout
            YUFU_BINARY="$bin"
            printf '%s\n' "$bin"
            return 0
        fi
        command -v cargo >/dev/null 2>&1 \
            || die "no $bin and cargo is not installed; set YUFU_BIN or build the checkout"
        log_info "building yufu from $root (first build can take several minutes)"
        (cd "$root" && cargo build --bin yufu) || die "cargo build failed in $root"
        [ -x "$bin" ] || die "cargo finished but $bin is missing"
        YUFU_BINARY_SOURCE=checkout
        YUFU_BINARY="$bin"
        printf '%s\n' "$bin"
        return 0
    fi

    bin=$(yufu_cached_bin "$version" "$target")
    if [ -n "$bin" ]; then
        YUFU_BINARY_SOURCE=cache
        YUFU_BINARY="$bin"
        printf '%s\n' "$bin"
        return 0
    fi

    need_cmd curl
    need_cmd tar
    sha_key="YUFU_SHA256_$(printf '%s' "$target" | tr '-' '_')"
    eval "expected=\${$sha_key:-}"
    [ -n "$expected" ] || die "no checksum recorded for $target ($sha_key)"
    url="https://github.com/Bin-hy/yufu/releases/download/$version/yufu-$version-$target.tar.gz"

    mkdir -p "$YUFU_WORKDIR"
    tmp=$(mktemp -d "$YUFU_WORKDIR/download.XXXXXX")
    log_info "downloading $url"
    if ! curl -fsSL --retry 2 --connect-timeout 10 -o "$tmp/archive.tar.gz" "$url"; then
        rm -rf "$tmp"
        die "download failed: $url; provide a local binary with YUFU_BIN"
    fi
    actual=$(sha256_of "$tmp/archive.tar.gz")
    if [ "$actual" != "$expected" ]; then
        rm -rf "$tmp"
        die "checksum mismatch for $target: expected $expected, got $actual"
    fi
    if ! tar -xzf "$tmp/archive.tar.gz" -C "$tmp"; then
        rm -rf "$tmp"
        die "failed to extract $url"
    fi
    candidate="$tmp/yufu-$version-$target/yufu"
    if [ ! -f "$candidate" ]; then
        rm -rf "$tmp"
        die "archive does not contain yufu-$version-$target/yufu"
    fi
    chmod 700 "$candidate"
    local reported
    reported=$("$candidate" --version 2>/dev/null || true)
    case "$reported" in
        yufu\ *) ;;
        *)
            rm -rf "$tmp"
            die "downloaded binary failed its self-check: ${reported:-no output}"
            ;;
    esac
    dest="$YUFU_WORKDIR/bin/yufu-$version-$target"
    mkdir -p "$dest"
    mv "$candidate" "$dest/yufu.new.$$"
    chmod 700 "$dest/yufu.new.$$"
    mv -f "$dest/yufu.new.$$" "$dest/yufu"
    rm -rf "$tmp"
    log_info "verified $target binary ($version)"
    YUFU_BINARY_SOURCE=download
    YUFU_BINARY="$dest/yufu"
    printf '%s\n' "$dest/yufu"
}

compose_cmd() {
    if docker compose version >/dev/null 2>&1; then
        COMPOSE=(docker compose)
    elif command -v docker-compose >/dev/null 2>&1; then
        COMPOSE=(docker-compose)
    else
        die "docker compose is required for the local database"
    fi
}

pg_running() {
    docker inspect -f '{{.State.Running}}' postgres_yufu 2>/dev/null | grep -qx true
}

pg_published_port() {
    local running
    running=$(docker inspect -f '{{.State.Running}}' postgres_yufu 2>/dev/null || true)
    [ -n "$running" ] || return 1
    if [ "$running" = "true" ]; then
        docker inspect -f '{{range $p, $conf := .NetworkSettings.Ports}}{{range $conf}}{{println .HostPort}}{{end}}{{end}}' \
            postgres_yufu 2>/dev/null | awk 'NF {print; exit}'
    else
        docker inspect -f '{{range $p, $conf := .HostConfig.PortBindings}}{{range $conf}}{{println .HostPort}}{{end}}{{end}}' \
            postgres_yufu 2>/dev/null | awk 'NF {print; exit}'
    fi
    return 0
}

pg_ready() {
    docker exec postgres_yufu pg_isready -U postgres >/dev/null 2>&1
}

# ensure_postgres COMPOSE_FILE PROJECT PORT
# Reuses the fixed-name container when possible, starts a stopped one, and
# only creates it through compose when it does not exist.
ensure_postgres() {
    local compose_file="$1" project="$2" port="$3"
    local mapped i

    need_cmd docker
    compose_cmd

    if docker inspect postgres_yufu >/dev/null 2>&1; then
        mapped=$(pg_published_port || true)
        if [ -n "$mapped" ] && [ "$mapped" != "$port" ]; then
            die "postgres_yufu is bound to host port ${mapped:-unknown} but YUFU_PG_PORT=$port; stop it or set YUFU_PG_PORT=$mapped"
        fi
        if pg_running; then
            log_info "reusing Postgres on 127.0.0.1:$port"
        else
            log_info "starting the existing Postgres container"
            docker start postgres_yufu >/dev/null || die "docker start postgres_yufu failed"
        fi
    else
        log_info "creating Postgres on 127.0.0.1:$port"
        if [ -n "$project" ]; then
            if ! POSTGRES_YUFU_PORT="$port" "${COMPOSE[@]}" -f "$compose_file" -p "$project" up -d postgres_yufu; then
                log_fail "Postgres container failed to start"
                log_info "check the port: lsof -nP -iTCP:$port -sTCP:LISTEN"
                log_info "check the logs: docker logs postgres_yufu"
                exit 1
            fi
        else
            if ! POSTGRES_YUFU_PORT="$port" "${COMPOSE[@]}" -f "$compose_file" up -d postgres_yufu; then
                log_fail "Postgres container failed to start"
                log_info "check the port: lsof -nP -iTCP:$port -sTCP:LISTEN"
                log_info "check the logs: docker logs postgres_yufu"
                exit 1
            fi
        fi
    fi

    i=0
    while [ "$i" -lt 60 ]; do
        if pg_ready; then
            return 0
        fi
        i=$((i + 1))
        sleep 1
    done
    die "Postgres did not become ready; check: docker logs postgres_yufu"
}

# browser_login ISSUER USER PASSWORD JAR
# Logs in with the console API and echoes the login response body.
# The cookie jar keeps the session for follow-up admin calls.
browser_login() {
    local issuer="$1" user="$2" pass="$3" jar="$4"
    local csrf body
    csrf=$(curl -sS -m 10 -c "$jar" -b "$jar" -H "Origin: $issuer" \
        "$issuer/api/v1/identity/csrf" 2>/dev/null || true)
    if [ -z "$csrf" ]; then
        die "csrf request failed at $issuer/api/v1/identity/csrf"
    fi
    csrf=$(printf '%s' "$csrf" | json_get csrf_token 2>/dev/null || true)
    [ -n "$csrf" ] || die "csrf response did not contain csrf_token"

    body=$(python3 -c 'import json,sys; print(json.dumps({"username": sys.argv[1], "password": sys.argv[2]}))' "$user" "$pass")
    local response status
    response=$(curl -sS -m 10 -c "$jar" -b "$jar" -X POST \
        -H "Origin: $issuer" -H "Content-Type: application/json" \
        -H "x-csrf-token: $csrf" -d "$body" \
        -w '\n%{http_code}' "$issuer/api/v1/identity/login" 2>/dev/null || true)
    status=$(printf '%s' "$response" | tail -n1)
    body=$(printf '%s' "$response" | sed '$d')
    if [ "$status" != "200" ]; then
        die "login failed with HTTP $status: $(printf '%s' "$body" | head -c 300)"
    fi
    printf '%s' "$body"
}