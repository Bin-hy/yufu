# Local Deployment

Goal: a working loopback issuer on the development machine for registration and integration.
This does not cover TLS, DNS names, reverse proxies, or multi-machine deployments.

## Prerequisites

| Dependency | Purpose | If missing |
| --- | --- | --- |
| `docker` + `docker compose` | Run PostgreSQL | Blocked |
| `curl`, `python3`, `openssl`, `tar` | Verification, JSON, signing key, extraction | Blocked |
| Network access or `YUFU_BIN` | Obtain the yufu binary | One of the two |
| Outbound UDP/123 | Secure clock check | Service fails to start or health returns 503 |

Run:

```bash
bash "$SKILL_DIR/scripts/preflight.sh"
```

Exit code `0` means continue; `2` means blockers exist and the output lists the missing items.

## Deploy

```bash
bash "$SKILL_DIR/scripts/deploy-local.sh"
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

`deploy-local.sh` performs these steps:

1. If discovery at `http://127.0.0.1:14000` already returns the same issuer, reuse it.
2. Start (or reuse) `postgres_yufu`: inside the yufu repo it uses the root `compose.yaml`; installed copies use the skill's
   `assets/compose.postgres.yaml`. Host port `55433` (when busy, a free port is chosen and recorded in `state.env`).
3. Create the `yufu_onboarding` database if it does not exist.
4. Render the configuration from `assets/yufu.toml.tmpl` to `.yufu-onboarding/yufu.toml`.
5. Generate a 2048-bit PKCS#8 private key with `openssl genpkey` if no signing key exists.
6. Choose a binary: `YUFU_BIN` → a detected source artifact (build with the local Rust toolchain if absent)
   → local cache → download the pinned version for this platform from `versions.env` and verify its SHA-256.
7. Start in the background and poll discovery for up to 60 seconds.

## Sources and integrity

Binaries and Go modules are version-pinned, and checksums guarantee integrity:

- Binaries come from the pinned URL
  `https://github.com/Bin-hy/yufu/releases/download/<version>/yufu-<version>-<target>.tar.gz`;
  the version and per-platform SHA-256 values are pinned in `assets/versions.env`. Scripts access only that
  release URL and never fetch or execute remote scripts.
- The SHA-256 is verified before anything is written to `bin/` or started.
- A local artifact found in the repository takes precedence and triggers no download.
- The Go SDK version is pinned in `assets/versions.env`; the scaffold writes it to the generated `go.mod`,
  and the module checksum is recorded in `go.sum`.
- `BUILD-INFO.json` inside the archive records the tag, version, commit, and target platform; to trace
  provenance, compare its commit with the commit behind that tag on the releases page.

The current publisher is a personal GitHub account (`Bin-hy`). Checksums prevent content substitution but
do not confirm the publisher's identity; regulated environments should evaluate this source against their
own policy.

## Overridable variables

| Variable | Default | Description |
| --- | --- | --- |
| `YUFU_PORT` | `14000` | yufu listen port |
| `YUFU_ISSUER` | derived from `YUFU_PORT` | issuer written to the config and state |
| `YUFU_PG_PORT` | `55433` | Postgres host port |
| `YUFU_DB_NAME` | `yufu_onboarding` | Database name |
| `YUFU_ADMIN_USERNAME` | `onboard-admin` | Bootstrap admin user |
| `YUFU_ADMIN_PASSWORD` | `onboard-admin-password` | Loopback only |
| `YUFU_BIN` | empty | Use an existing binary (works offline) |
| `YUFU_TARGET` | derived from `uname` | Override the platform; must be in the supported list |
| `YUFU_ROOT` | auto-detected | Explicit yufu source checkout |
| `YUFU_APP_PORT` | chosen by `register-app.sh` | Loopback port for the demo web app |
| `YUFU_WORKDIR` | `./.yufu-onboarding` | State, config, logs, binary cache |

When `YUFU_PORT` / `YUFU_PG_PORT` are not set explicitly, the scripts increment past busy default ports and
write the final values to `state.env`. Explicit values are used strictly; a port conflict fails immediately.

## Port conflicts

The scripts never stop services already running on the machine. When a default port is busy:

- `YUFU_PORT` (default `14000`) is busy: use the next free port; the issuer changes and is written to `state.env`.
- `YUFU_PG_PORT` (default `55433`) is busy: reuse the published `postgres_yufu` when possible, otherwise use the next free port.
- Explicit `YUFU_PORT` or `YUFU_PG_PORT` values take precedence; if the port is still busy the script fails instead of changing it.

`register-app.sh` chooses a free web demo app port starting at `8080` and records it as `YUFU_APP_PORT`.
Start the app with `ADDR=127.0.0.1:$YUFU_APP_PORT`, and keep the callback address byte-for-byte identical to the registered value.

### Issuer and database binding

yufu pins the issuer from its first startup in `identity_provider`. An initialized database **cannot** be
reused on a different port or issuer — the service exits during startup because the issuer does not match.
The script checks this before starting and fails immediately. Three options:

- Reuse the original `YUFU_PORT` / `YUFU_ISSUER`.
- Switch to a different `YUFU_DB_NAME` (a fresh database).
- If the old data is explicitly expendable, delete the old database and recreate it.

Do not restart the same configuration repeatedly after this error; `identity initialization failed` in the log is the symptom.

## Readiness criteria

All three must hold:

- The `issuer` from discovery matches the configured value byte for byte.
- JWKS contains at least one key.
- `/api/v1/health/ready` returns 200 with `security_clock_synchronized = true`.

`verify-issuer.sh` prints `[ok]` / `[fail]` for each item.

## Stop and clean up

```bash
kill "$(sed -n 's/^YUFU_SERVER_PID=//p' .yufu-onboarding/state.env)"
docker stop postgres_yufu
```

Database volumes are preserved. Deleting a volume is explicit destructive cleanup: confirm first.

## Reuse rules

Running `deploy-local.sh` again does not recreate the database and does not restart an already-ready service.
It rewrites the configuration only when it actually starts the process, and it starts the process only when discovery is unavailable.
