# Troubleshooting

Locate the issue by stage; do not guess. Every entry includes a command you can run.

## Deployment

| Symptom | Likely cause | Check |
| --- | --- | --- |
| `preflight blocked: port 14000 is already in use` | A stale process or another service owns the port | `lsof -nP -iTCP:14000 -sTCP:LISTEN` |
| `Postgres did not become ready` | Container failed to start or port conflict | `docker logs postgres_yufu` |
| `postgres_yufu is bound to host port ...` | Existing container port differs from `YUFU_PG_PORT` | Stop the old container, or set `YUFU_PG_PORT=<old port>` and rerun |
| `download failed` | Restricted network or unreachable release | Point `YUFU_BIN` at a local binary, or use a proxy and rerun |
| `checksum mismatch` | Archive replaced or the version file is stale | Delete `.yufu-onboarding/bin/` and retry; verify `assets/versions.env` |
| `Migration file of version ... is missing` | The database was already migrated by a newer version, so the pinned old binary cannot open it | Rebuild with a separate `YUFU_DB_NAME`, or use a version that matches the migrations |
| `unsupported platform` or `unsupported YUFU_TARGET` | Platform is not in the supported list | Use one of `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu` |
| yufu exits immediately after starting | Missing key, bad config, or port conflict | `tail -50 .yufu-onboarding/yufu.log` |
| `health ready but security clock is not synchronized` | Outbound UDP/123 is blocked | `nc -zu time.cloudflare.com 123`; switch to an internal time source |
| Discovery issuer mismatch | The configured issuer differs from the reachable address | Rerun `deploy-local.sh`; do not edit the config by hand |

## Registration

| Symptom | Likely cause | Check |
| --- | --- | --- |
| `login failed with HTTP 401` | Admin account mismatch | Check `YUFU_ADMIN_USER` in `state.env` |
| `registration returned HTTP 400 invalid_input` | `server_web` was given `browser_origins`, or the redirect is invalid | Compare with `--mode api`, or re-check the parameter constraints in `references/register-app.md` |
| `registration returned HTTP 409` | `app_key` exists with a different idempotency key | Reuse the original `--app-key` |
| `client ... was registered without a new secret` | Replays do not return secrets | Reset the secret in the console, or register again with a new `--app-key` |

## Parameter pairing

| Symptom | Likely cause | Check |
| --- | --- | --- |
| `redirect_uri mismatch: registered ..., current ...` | An environment variable overrode the registered value | Align `YUFU_REDIRECT_URI` with `state.env` |
| `authorization endpoint returned HTTP 4xx` | Client does not exist or the redirect is not allowlisted | `bash "$SKILL_DIR/scripts/verify-app.sh"`; the response body names the error code |
| `authorization endpoint rejected the request` | Scope not authorized or the app has no access assignment | Confirm in the console that the user has an access assignment |
| `end-session endpoint returned HTTP 400` | The sign-out address was not also registered in `redirect_uris` (client registered by an older script) | Re-register with a new `--app-key`; `verify-app.sh` names the field |

## Integration

| Symptom | Likely cause | Check |
| --- | --- | --- |
| Callback returns 400 `unknown or replayed state` after sign-in | State was not persisted or the deployment spans multiple instances | Switch the session store to shared storage |
| Callback returns 401 `nonce does not match` | Verification used a new nonce | Use the nonce saved when the authorization request was started |
| API always returns 401 `audience does not match` | Verification used the client id instead of the access audience | Check `YUFU_AUDIENCE` |
| API returns intermittent 401s | JWKS rotation and caching | The SDK refetches by `kid`; confirm `JWKSTTL` is not 0 |
| `go: module lookup disabled` | Offline with an empty module cache | Run `go mod download` in a networked environment first |

## General

- State file: `.yufu-onboarding/state.env` (mode 600; never commit it).
- Service log: `.yufu-onboarding/yufu.log`.
- When any step fails, run `bash "$SKILL_DIR/scripts/verify-issuer.sh"` first to rule out issuer-level problems.
