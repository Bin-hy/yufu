---
name: yufu-onboarding
description: |
  Deploy a local yufu identity provider and integrate it into an existing application.
  Use this skill whenever the user wants to:
  - Deploy yufu and connect this project
  - Add SSO login to a Go service
  - Protect an API with yufu bearer tokens
  - Register a yufu application and verify redirect/audience parameters
  - Diagnose why a yufu login or token verification fails
  Triggers: yufu onboarding, deploy yufu, connect yufu, Go SDK integration, OIDC integration, SSO login, token verification.
  Do not use for production TLS/hosting setups, M2M client credentials, Python/Node SDK work,
  or changes to the yufu server or yufu-sdk-go themselves.
---

# yufu Onboarding

One entry point, five stages: preflight → deploy → register → integrate → verify.

Resolve the skill directory first, then call every script by absolute path; the scripts do not need the executable bit:

```bash
SKILL_DIR="<directory containing this SKILL.md>"
```

Keep `SKILL.md` language-agnostic. Language-specific details live in `references/`; load them per stage.

## Quick path (single request)

When the request is "deploy yufu and connect this project", skip the background and start executing.

**Ask one question, exactly once:**

> Does your application need users to sign in through yufu, or only verify yufu tokens on its own API?
> 1. Sign-in → web backend
> 2. Token verification only → API resource server

Then run one command that performs preflight, deployment, registration, and parameter verification:

```bash
# Option 1: sign-in
bash "$SKILL_DIR/scripts/onboard.sh" --mode web --project-dir "$PWD"

# Option 2: token verification only
bash "$SKILL_DIR/scripts/onboard.sh" --mode api --project-dir "$PWD"
```

Append flags for a custom callback, audience, or scopes; see `references/register-app.md`.
When it succeeds, read the handoff block, then load the stage 3 reference for the target language and write the integration code.

## Stage 0: Preflight

```bash
bash "$SKILL_DIR/scripts/preflight.sh"
```

- Exit code `0`: continue.
- Exit code `2`: blockers are listed. Fix them before continuing; do not bypass them.

Then read the state file (a missing file means a fresh start):

```bash
cat .yufu-onboarding/state.env 2>/dev/null || echo "no state"
```

Run:

```bash
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

- Pass: record `YUFU_ISSUER` and skip to stage 2.
- Fail: go to stage 1.

## Stage 1: Deploy

Load `references/deploy-local.md`, then run:

```bash
bash "$SKILL_DIR/scripts/deploy-local.sh"
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

Both must pass. `deploy-local.sh` is idempotent: when the issuer is already ready it only logs that it is reusing it.
Without a source checkout it downloads the pinned version and verifies the checksum, so Rust and cargo are not required.
For offline environments, point `YUFU_BIN` at a local binary.

## Stage 2: Register

If you entered at stage 0 and have not asked yet, ask this question once:

> Does your application need users to sign in through yufu, or only verify yufu tokens on its own API?
> 1. Sign-in → web backend
> 2. Token verification only → API resource server

Then run one of:

```bash
bash "$SKILL_DIR/scripts/register-app.sh" --mode web
bash "$SKILL_DIR/scripts/register-app.sh" --mode api
```

The script handles csrf → login → app creation → parameter pairing check.
For a custom redirect URI, audience, or scopes, read `references/register-app.md` first.

## Stage 3: Integrate

| Choice | Load | Done when |
| --- | --- | --- |
| Web backend | `references/go-web.md` | `go build ./...` passes and a completed sign-in exposes `sub` |
| API resource server | `references/go-resource-server.md` | `go test ./...` passes and the 401/200 behavior is correct |

Add only the files the integration needs and confirm the change scope with the application owner.
Do not reorganize existing directories, introduce a web framework, or modify the yufu server or SDK.

## Stage 4: Verify

Load `references/verification.md` and collect evidence for all three behaviors:

1. Sign-in: `verify-app.sh` passes, and a human completes one browser sign-in and sees `sub`.
2. Token: the generated `go test` passes; no token returns 401, a valid token returns 200.
3. Sign-out: after visiting `/logout`, the protected page requires sign-in again.

## Failure handling

Load `references/troubleshooting.md` and locate the issue by stage → symptom.
Run `verify-issuer.sh` first to rule out issuer-level problems, then inspect the application layer.
Each script names the failing stage and prints at least one actionable diagnostic command.

## Ports

When a default port is in use, the scripts choose the next free port and record the final value in `state.env`; they never stop other containers, tunnels, or services.
Explicit `YUFU_PORT` / `YUFU_PG_PORT` settings always win.

`register-app.sh` chooses the web demo app port starting at `8080` and records it as `YUFU_APP_PORT`.
Start the app with `ADDR` set to that address, and keep `REDIRECT_URI` byte-for-byte identical to the registered value.

The database pins the issuer from its first startup, so an initialized database fails to start on a different port; `deploy-local.sh` detects this and fails early.
Reuse the original port, or switch to a new `YUFU_DB_NAME`.

## State and files

- State: `.yufu-onboarding/state.env` (mode 600; never commit it; add `.yufu-onboarding/` to the project `.gitignore`).
- Logs: `.yufu-onboarding/yufu.log`.
- Override the working directory with `YUFU_WORKDIR`.

## Never

- Never print or commit the client secret; it exists only in `.yufu-onboarding/client_secret` or the server environment.
- Never use `@latest`; SDK versions are pinned in `assets/versions.env`.
- Download binaries only from the GitHub Release URLs pinned in `assets/versions.env` and verify SHA-256; never execute remote scripts.
- Never modify the `yufu/` server code or the `yufu-sdk-go` repository.
- Never treat a loopback configuration as production configuration.
- Never delete database volumes, configuration, or existing applications without consent.

## Adding a language

Add `references/<lang>-web.md`, `references/<lang>-resource-server.md`, and any required `assets/<lang>/`, then add a row to the stage 3 table.
Do not change stages 0–2 or the verification behavior.
