# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

CoBotrix is a cloud platform for remote smart devices (robotics, IoT). It provides unified networking, API, auth, and device management.

**Components**:
- **cobotsrv** — OIDC identity provider (IdP) + policy decision point (PDP) + admin backend
- **cobotrix** — device management center (API + SPA), independent JWT verification via JWKS
- **cobotlet** — device agent (runs on each edge device, MQTT comms)
- **cobotnil** — dev cluster launcher (starts cobotsrv + cobotrix + cobotlets + MQTT broker)

**Deployment modes** (see README.md for full details):
- **Mode A** (embedded): Frontend baked into binary via `rust-embed` — build frontend first, then `cargo build --release --features webui --bin cobotsrv`
- **Mode B** (split): Separate static frontend + API backend without `webui` feature
- **Mode C** (dev): `cargo run --bin cobotnil -- -c conf/cobotnil` for backend cluster + `npm run dev` in `cobotweb/` for frontend hot reload

## Workspace layout

```
cobotrix/              # root crate — binary entrypoints only
  src/cobotnil.rs      # dev cluster launcher (cobotsrv + cobotrix + cobotlets + MQTT)
  src/cobotsrv.rs      # cobotsrv binary
  src/cobotrix.rs      # cobotrix binary
  src/cobotlet.rs      # cobotlet binary
  src/utils.rs         # shared utils (config loader, shutdown signal)
cobotsrv/              # cobotsrv library — HTTP API, OIDC, DB, PDP
  src/entity/          # SeaORM entities (17 tables)
  src/route/           # route handlers (auth + public + no-prefix groups)
  src/service/         # business logic layer
  src/migration/       # SeaORM migrations
  src/seed/            # seed data
cobotrix/              # cobotrix library — device/task API, MQTT, JWT verification
  src/route/           # device, task, remote, hello routes
  src/service/         # device service, database
  src/entity/          # device entity
  src/mqtt.rs          # MQTT client to cobotlets
cobotlet/              # cobotlet library — runs on each device, MQTT comms
  src/route/           # device, task routes
  src/store/           # local DB store
  src/mqtt.rs          # MQTT client
  src/heartbeat.rs     # device heartbeat
  src/dispatcher.rs    # task dispatch
cobotweb/              # Vue 3 monorepo (pnpm workspace)
  packages/shared/     # shared types, HTTP client factory, cache utils
  apps/cobotsrv/       # cobotsrv management SPA (auth, users, roles, permissions, apps)
  apps/cobotrix/       # cobotrix management SPA (dashboard, devices, tasks)
  apps/cobotlet/       # cobotlet management SPA (device detail, task dispatch)
libs/auth/             # JWT/IdToken signing and verification (RSA + HMAC)
conf/                  # config templates (*.example tracked, *.toml/.local ignored)
  cobotnil/            # dev cluster configs (cobotsrv + cobotrix + cobotlets)
tests/                 # integration tests (testcontainers + Postgres)
  cobotsrv/
    common.rs          # test helpers (postgres container, token generation)
    entity/            # DB entity tests
    routes/            # per-route-module integration tests
```

## Build & test commands

```bash
# Format check
cargo fmt --all -- --check

# Clippy (treat all warnings as errors)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build everything
cargo build --workspace --all-targets --all-features

# Run all tests (reduced concurrency — testcontainers flake on port conflicts)
cargo test --workspace --all-targets --all-features -- --test-threads=4

# Run a single integration test
cargo test --test tests test_name -- --test-threads=1

# Frontend
cd cobotweb && pnpm install && pnpm run dev          # dev server (all apps)
cd cobotweb && npx vue-tsc -b && pnpm run build      # typecheck + build
```

## Feature flags

| Flag | Effect |
|------|--------|
| `net4mqtt` (default) | MQTT-based reverse proxy for device ↔ server communication |
| `webui` | Embed `assets/cobotsrv/` into `cobotsrv` binary via `rust-embed` |
| `openapi` | Enable Swagger UI at `/docs` and utoipa API docs generation |

## Route architecture

Three route groups assembled in `cobotsrv/src/lib.rs::serve()`:

1. **`get_auth_routes()`** — mounted under `<api_prefix>` (default `/api/v1`). Requires JWT. Wrapped in: recording middleware → RBAC/ABAC authority check → JWT access middleware with user-injection callback. Modules: `user`, `auth`, `role`, `authority`, `recording`, `user_authority`, `oidc` (authorize, userinfo), `oidc_admin` (client/scope management), `enforce` (PDP), `application`, `permission`, `role_permission`, `user_permission`.

2. **`get_public_routes()`** — mounted under `<api_prefix>`. No auth required. Modules: `user`, `auth`, `rbac`, `sync_permissions`.

3. **`get_no_prefix_routes()`** — mounted at root (no prefix). Modules: `oidc` public endpoints (discovery, JWKS, token, revoke, introspect).

CORS is configured from `[auth].origins` in the config. Wildcard `"*"` uses `AllowOrigin::mirror_request()`.

## Permission system

Two parallel systems, intentionally separate:

| System | Tables | Granularity | Use case |
|--------|--------|-------------|----------|
| **authority** | `authority`, `authority_role`, `user_authority` | URI + Method | cobotsrv's own API access control |
| **permission** | `permission`, `role_permission`, `user_permission` | action + resource + glob pattern | Business app permissions (e.g. cobotrix) |

Permission flow:
1. Admin registers an Application + OAuth Client in `/applications`
2. Admin defines Permissions (name, action, resource_type, resource_pattern) in `/permissions`
3. Admin assigns permissions to roles in `/permissions/assign`
4. At token issuance, `resolve_user_permissions()` embeds `permissions: ["device:read", ...]` in JWT claims
5. cobotrix uses `require_perm!("device:read")` macro for O(1) local enforcement
6. PDP endpoint `POST /api/v1/enforce` for fine-grained glob pattern matching

## OIDC implementation

- Endpoints split across `route/oidc.rs` (public + auth) and `route/oidc_admin.rs` (admin)
- `IdTokenSigner` in `libs/auth` supports RSA (RS256) and HMAC (HS256) fallback
- RSA key auto-generated on first start, persisted to `conf/oidc_rsa_private.pem`
- Token expiry: access tokens use `oidc.access_token_expire_seconds` with fallback to `auth.exp_seconds`; refresh tokens use `refresh_exp_seconds` similarly
- OIDC public routes (discovery, JWKS, token, revoke, introspect) are mounted at root level without the API prefix, as required by OIDC specs (RFC 8414, RFC 7517, etc.)
- PKCE (S256) supported for SPA public clients — cobotrix uses authorization_code + PKCE
- Bootstrap clients via `[[oidc.bootstrap_clients]]` auto-create OAuth Client + Application at startup

## cobotrix architecture

- Independent JWT verification via JWKS (RS256) — no shared secret with cobotsrv
- `require_perm!` macro reads permissions from token claims at route level
- `sync_permissions()` at startup: POSTs required permissions to cobotsrv via client_credentials
- SPA calls cobotrix API directly (dev: Vite proxy `/devices`,`/tasks` → cobotrix:4001, `/oidc` → cobotsrv:4000)
- User info displayed in SPA header via OIDC `/userinfo` endpoint

## Database

SeaORM 2.0-rc.2 with PostgreSQL. Migrations in `cobotsrv/src/migration/`. Entities in `cobotsrv/src/entity/`. Seed data in `cobotsrv/src/seed/`. Integration tests spin up Postgres via testcontainers.

Key entities (17 tables): `user`, `role`, `authority`, `authority_role`, `user_authority`, `recording`, `oauth_client`, `oauth_scope`, `oauth_token`, `oauth_authorization_code`, `oauth_consent`, `refresh_token`, `application`, `permission`, `role_permission`, `user_permission`.

## Config setup (first clone)

```bash
cp .env.example .env
cp compose.example.yml compose.local.yml
cp conf/cobotsrv.example.toml conf/cobotsrv.local.toml
cp conf/cobotrix.example.toml conf/cobotrix.local.toml
cp conf/cobotlet.example.toml conf/cobotlet.local.toml
cp cobotweb/.env.example cobotweb/.env.local
```

`*.local`, `*.local.*`, `.env`, and `conf/oidc_rsa_private*.pem` are gitignored.

## Testing notes

- Integration tests require Docker daemon running (testcontainers for Postgres)
- Tests use `portpicker` to find free ports for the HTTP server
- Test concurrency (`--test-threads=4`) is important — Docker port exposure can flake under full parallelism
- `tests/cobotsrv/common.rs` provides `get_postgres_container()` and `get_test_token()` helpers
