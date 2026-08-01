# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

CoBotrix (Co-Robotics Matrix) is a cloud platform for remote smart devices (robotics, IoT). It provides unified networking, API, auth, and device management.

> **Naming note (important):** the two primary backend crates were renamed/restructured. `cobotusr` is the auth/identity backend (formerly "cobotsrv"), and `cobotsrv` is now the device-management backend (formerly "cobotrix"). Older docs/branches may use the old names — trust this file and the code.

**Crates / components**:
- **cobotusr** — OIDC identity provider (IdP) + policy decision point (PDP) + admin backend. Owns the users/roles/permissions/OAuth database. Also hosts an instant-messaging (`im`) module. Config `cobotusr.toml`, SPA `cobotweb/apps/cobotusr`.
- **cobotsrv** — device-management center: device/task API, MQTT bridge to cobotlets, MCP server, easytier networking. Independent JWT verification via JWKS (no shared secret with cobotusr). Config `cobotsrv.toml`, SPA `cobotweb/apps/cobotsrv`.
- **cobotlet** — device agent running on each edge device (MQTT comms). SPA `cobotweb/apps/cobotlet`.
- **cobotnet** — easytier (P2P VPN) integration library: starts an easytier node, peer discovery, virtual-IP allocation. Gated behind the `easytier` feature; consumed by cobotsrv and cobotlet.
- **cobotset** — CLI that registers a device's capabilities (HTTP method/path/tags) to a cobotlet.
- **cobotmap** — standalone reverse proxy for the Google Maps JS API (see `cobotmap/README.md`).
- **cobotrix** — combined binary launching `cobotusr` + `cobotsrv` together (config dir `conf/cobotrix/`).
- **cobotnil** — dev cluster launcher: embedded net4mqtt broker + cobotusr + cobotsrv + cobotlets (config dir `conf/cobotnil/`).

**Deployment modes** (see README.md for full details):
- **Mode A** (embedded): frontend baked into the binary via `rust-embed` — build frontend first, then `cargo build --release --features webui`
- **Mode B** (split): separate static frontend + API backend, no `webui` feature
- **Mode C** (dev): `cargo run --bin cobotnil -- -c conf/cobotnil` for the backend cluster + `pnpm run dev` in `cobotweb/` for frontend hot reload

## Workspace layout

The root crate (`cobotrix-root`) holds only binary entrypoints in `src/`; each service is its own workspace member library.

```
src/                   # binary entrypoints only (Cargo.toml [[bin]] targets)
  cobotnil.rs          # dev cluster launcher (broker + cobotusr + cobotsrv + cobotlets)
  cobotrix.rs          # combined launcher (cobotusr + cobotsrv)
  cobotusr.rs          # cobotusr binary
  cobotsrv.rs          # cobotsrv binary
  cobotlet.rs          # cobotlet binary
  cobotset.rs          # cobotset CLI binary
  cobotmap.rs          # cobotmap proxy binary (+ cobotmap_verify.rs)
  utils.rs / log.rs    # shared config loader, shutdown signal, logging
cobotusr/              # IdP/PDP/admin library — OIDC, RBAC/ABAC, permissions, DB, im
  src/route/           # auth + public + no-prefix route groups
  src/service/         # business logic layer
  src/modules/im/      # instant-messaging module (controller/model/service/router)
  src/entity/ migration/ seed/   # SeaORM entities, migrations, seed data
cobotsrv/              # device-management library — device/task API, MQTT, MCP, easytier
  src/route/           # device, task, remote, hello, cobotlet, easytier_config, tool
  src/mcp/             # MCP Streamable HTTP server (protocol, handler, route, sse)
  src/service/         # device service, network (easytier), database
  src/entity/          # device, easytier_network
  src/mqtt.rs          # MQTT client to cobotlets
cobotlet/              # device agent — MQTT comms, local store, heartbeat, dispatcher
cobotnet/              # easytier VPN node lifecycle + peer events
cobotset/ cobotmap/    # capability-registration CLI / maps proxy
cobotweb/              # Vue 3 monorepo (pnpm workspace)
  packages/shared/     # shared types, HTTP client factory, cache utils
  apps/{cobotusr,cobotsrv,cobotlet}/   # one SPA per backend
libs/auth/             # JWT/IdToken signing & verification, require_perm! macro, access middleware
conf/                  # config templates (*.example tracked, *.toml/*.local ignored)
  cobotnil/ cobotrix/  # multi-service dev config dirs
tests/cobotsrv/        # integration tests (testcontainers + Postgres): common.rs, mcp.rs
tests/cobotusr/        # cobotusr integration tests: common.rs, routes/, entity/, im/
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

Two project skills wrap the CI gates: **`ci-check-back`** (backend Rust: fmt → clippy → build → test) and **`ci-check-web`** (cobotweb build). Prefer them when running the full check sequence.

## Feature flags

Workspace features fan out to the member crates (see root `Cargo.toml [features]`):

| Flag | Effect |
|------|--------|
| `net4mqtt` (default) | MQTT-based reverse proxy for device ↔ server communication |
| `webui` | Embed each SPA into its binary via `rust-embed` (cobotlet, cobotusr, cobotsrv) |
| `openapi` | Swagger UI + utoipa API docs (cobotusr only) |
| `easytier` | P2P VPN networking via cobotnet (cobotlet + cobotsrv) |

## Route architecture

**cobotusr** (`cobotusr/src/route/mod.rs`, assembled in `cobotusr/src/lib.rs::serve()`):
1. **`get_auth_routes()`** — under `<api_prefix>` (default `/api/v1`), requires JWT. Wrapped: recording middleware → RBAC/ABAC authority check → JWT access middleware with user-injection. Modules: `user`, `auth`, `role`, `authority`, `recording`, `user_authority`, `oidc` (authorize, userinfo), `oidc_admin`, `enforce` (PDP), `application`, `permission`, `role_permission`, `user_permission`, plus the `im` router.
2. **`get_public_routes()`** — under `<api_prefix>`, no auth. Modules: `user`, `auth`, `rbac`, `sync_permissions`.
3. **`get_no_prefix_routes()`** — at root, no prefix. OIDC public endpoints (discovery, JWKS, token, revoke, introspect) per OIDC specs (RFC 8414, RFC 7517).

**cobotsrv** (`cobotsrv/src/route/mod.rs`, assembled in `cobotsrv/src/lib.rs::serve()`):
- **`get_protected_routes()`** — device/task/remote/cobotlet/easytier_config/tool, JWT-verified via JWKS + `require_perm!`.
- **`get_public_routes()`** — `hello` and other unauthenticated endpoints.
- MCP server mounted at `/mcp` (see below).

CORS is configured from `[auth].origins`. Wildcard `"*"` uses `AllowOrigin::mirror_request()`.

## MCP server (cobotsrv)

cobotsrv exposes an MCP 2025-03-26 Streamable HTTP server at `/mcp` (`cobotsrv/src/mcp/`):
- `GET /mcp` → SSE stream (`sse.rs`); `POST /mcp` → JSON-RPC 2.0 (`route.rs` → `handler::dispatch`).
- Batch requests (JSON arrays) are rejected; notifications (no `id`) return `202 Accepted` with no body.
- `POST /mcp` (protocol endpoint) is guarded by `require_perm!("mcp:access")`; the admin endpoints under `/mcp/admin/*` (including `GET /mcp/admin/tools`, the aggregated tool list) are guarded by `require_perm!("mcp:admin")`. Protocol types live in `protocol.rs`.
- MCP access tokens are issued/managed by cobotusr under `/oidc/mcp-tokens` (`POST /oidc/mcp-tokens` issue, `POST /oidc/mcp-tokens/introspect`, `GET/PATCH/DELETE /oidc/mcp-tokens[/{id}]`, `POST /oidc/mcp-tokens/{id}/refresh`); each user is limited to 20 active tokens, and expired tokens are not listed as active.
- MCP tool calls are audited to the `mcp_call_log` table (composite index on `(subject, created_at)`); a daily job purges rows older than 30 days.

## Permission system

Two parallel systems in **cobotusr**, intentionally separate:

| System | Tables | Granularity | Use case |
|--------|--------|-------------|----------|
| **authority** | `authority`, `authority_role`, `user_authority` | URI + Method | cobotusr's own API access control |
| **permission** | `permission`, `role_permission`, `user_permission` | action + resource + glob pattern | Business app permissions (e.g. cobotsrv) |

Permission flow:
1. Admin registers an Application + OAuth Client in `/applications`
2. Admin defines Permissions (name, action, resource_type, resource_pattern) in `/permissions`
3. Admin assigns permissions to roles in `/permissions/assign`
4. At token issuance, `resolve_user_permissions()` embeds `permissions: ["device:read", ...]` in JWT claims
5. cobotsrv uses the `require_perm!("device:read")` macro (from `libs/auth`) for O(1) local enforcement
6. PDP endpoint `POST /api/v1/enforce` (cobotusr) for fine-grained glob pattern matching

## OIDC implementation (cobotusr)

- Endpoints split across `route/oidc.rs` (public + auth) and `route/oidc_admin.rs` (admin)
- `IdTokenSigner` in `libs/auth` supports RSA (RS256) and HMAC (HS256) fallback
- RSA key auto-generated on first start, persisted to `conf/oidc_rsa_private.pem` (gitignored — never commit)
- Token expiry: access tokens use `oidc.access_token_expire_seconds` (fallback `auth.exp_seconds`); refresh tokens use `refresh_exp_seconds` similarly
- OIDC public routes are mounted at root (no API prefix), as required by OIDC specs
- PKCE (S256) supported for SPA public clients (authorization_code + PKCE)
- Bootstrap clients via `[[oidc.bootstrap_clients]]` auto-create OAuth Client + Application at startup

## cobotsrv ↔ cobotusr integration

- cobotsrv verifies JWTs independently via JWKS (RS256) — no shared secret with cobotusr
- `require_perm!` reads `permissions` from token claims at the route level
- `sync_permissions()` at startup POSTs cobotsrv's required permissions to cobotusr via client_credentials
- SPAs call their backend API directly; cobotsrv SPA gets user info via cobotusr's OIDC `/userinfo`

## easytier networking (cobotnet)

- `cobotnet` wraps an easytier `Instance`: starts a VPN node, emits `PeerEvent::Online/Offline`, allocates virtual IPs
- Wired into `cobotsrv/src/lib.rs::serve()` behind `#[cfg(feature = "easytier")]`; config under `[easytier]` in `cobotsrv.toml`
- `NetworkService` (`cobotsrv/src/service/network.rs`) persists allocations to the `easytier_network` table and runs a reclaim task

## Database

SeaORM 2.0-rc.2 with PostgreSQL. The authoritative schema lives in **cobotusr**: migrations in `cobotusr/src/migration/`, entities in `cobotusr/src/entity/`, seed in `cobotusr/src/seed/`. cobotsrv has its own small schema (`device`, `easytier_network`). Integration tests spin up Postgres via testcontainers.

Core cobotusr entities: `user`, `role`, `authority`, `authority_role`, `user_authority`, `recording`, `oauth_client`, `oauth_scope`, `oauth_token`, `oauth_authorization_code`, `oauth_consent`, `refresh_token`, `application`, `permission`, `role_permission`, `user_permission`.

## Config setup (first clone)

All `*.example` and `*.local.*` files are templates; copy them to local (gitignored) names before running:

```bash
cp .env.example .env
cp conf/cobotusr.example.toml conf/cobotusr.local.toml
cp conf/cobotsrv.example.toml conf/cobotsrv.local.toml
cp conf/cobotlet.example.toml conf/cobotlet.local.toml
cp conf/cobotmap.example.toml conf/cobotmap.toml
cp cobotweb/.env.example cobotweb/.env.local
```

`*.local`, `*.local.*`, `.env`, and `conf/oidc_rsa_private*.pem` are gitignored. The RSA key is generated automatically on first cobotusr start.

## Testing notes

- Integration tests require a running Docker daemon (testcontainers for Postgres)
- Tests use `portpicker` to find free HTTP ports
- Test concurrency (`--test-threads=4`) matters — Docker port exposure flakes under full parallelism
- `tests/cobotsrv/common.rs` provides `start()` (returns `CobotsrvTestCtx`) and `insert_test_device()`; `tests/cobotusr/common.rs` provides `get_postgres_container()` and `get_test_token()` helpers
