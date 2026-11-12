#!/bin/sh
# yufu container entrypoint.
#
# Official images are configured with environment variables, so this script
# turns them into a working configuration on first start. Mounting a
# configuration yourself always wins and is never rewritten: that is the
# production path and it stays byte-for-byte what you wrote.
set -eu

MOUNTED_CONFIG="${YUFU_CONFIG:-/etc/yufu/yufu.toml}"
STATE_DIR="${YUFU_STATE_DIR:-/var/lib/yufu}"
GENERATED_CONFIG="$STATE_DIR/yufu.toml"
KEY_PATH="$STATE_DIR/oidc_rsa_private.pem"
BIN=/usr/local/bin/yufu

# 1. Operator-supplied configuration: run it untouched.
if [ -f "$MOUNTED_CONFIG" ]; then
  exec "$BIN" --config "$MOUNTED_CONFIG" "$@"
fi

# 2. No configuration: render one from the environment.
: "${YUFU_ISSUER:?set YUFU_ISSUER (for example https://id.example.com), or mount a configuration at $MOUNTED_CONFIG}"
: "${AUTH_SECRET:?set AUTH_SECRET to a long random value, or mount a configuration at $MOUNTED_CONFIG}"
: "${DATABASE_URL:?set DATABASE_URL (postgres://user:password@host:5432/db), or mount a configuration at $MOUNTED_CONFIG}"

PORT="${PORT:-4000}"
ADMIN_USER="${YUFU_ADMIN_USERNAME:-sadmin}"
ADMIN_PASS="${YUFU_ADMIN_PASSWORD:-sadmin}"
ADMIN_EMAIL="${YUFU_ADMIN_EMAIL:-}"
ORIGINS="${YUFU_ORIGINS:-$YUFU_ISSUER}"

mkdir -p "$STATE_DIR"

if [ ! -f "$GENERATED_CONFIG" ]; then
  (
    umask 077
    cat >"$GENERATED_CONFIG" <<EOF
# Written by the image entrypoint on first start, from environment variables.
# Mount your own file at $MOUNTED_CONFIG to take over; this one is then ignored.
#
# AUTH_SECRET and DATABASE_URL are deliberately absent: the service reads both
# from the environment, so no secret is written to persistent storage here.

[http]
listen = "0.0.0.0:$PORT"

[auth]
origins = ["$ORIGINS"]

[auth.super_admin]
username = "$ADMIN_USER"
password = "$ADMIN_PASS"
EOF
    if [ -n "$ADMIN_EMAIL" ]; then
      printf 'email = "%s"\n' "$ADMIN_EMAIL" >>"$GENERATED_CONFIG"
    fi
    cat >>"$GENERATED_CONFIG" <<EOF

[oidc]
issuer = "$YUFU_ISSUER"
rsa_private_key_path = "$KEY_PATH"
EOF
  )
  echo "[entrypoint] rendered $GENERATED_CONFIG from the environment" >&2
fi

# 3. Signing key. Generated only in this no-configuration mode; a mounted
#    configuration is expected to point at a key the operator owns.
if [ ! -f "$KEY_PATH" ]; then
  echo "[entrypoint] generating a 2048-bit RS256 signing key at $KEY_PATH" >&2
  echo "[entrypoint] WARNING: this key is for evaluation; production should mount its own" >&2
  openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$KEY_PATH"
  chmod 600 "$KEY_PATH"
fi

if [ "$ADMIN_PASS" = "sadmin" ] && [ "$(id -u)" = "0" ]; then
  echo "[entrypoint] WARNING: the super administrator still uses the default password" >&2
fi

exec "$BIN" --config "$GENERATED_CONFIG" "$@"
