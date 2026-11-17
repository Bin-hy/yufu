# Verification

Verification relies only on protocol behavior, not console pages. Integration is complete when all three
behaviors hold.

## Behavior 1: Sign-in

| Observation | Expected |
| --- | --- |
| Visit `/login` | 302 to the issuer's `/oidc/authorize` with `state`, `nonce`, `code_challenge`, `code_challenge_method=S256` |
| Sign in and consent | The callback carries `code` and the same `state` |
| Callback completes | A local session is established and a stable `sub` is readable |

Command:

```bash
bash "$SKILL_DIR/scripts/verify-app.sh"
```

It requests the authorization endpoint with an authenticated session and checks that client, redirect, and
scopes match. A 2xx/3xx response passes.

## Behavior 2: Token verification

| Observation | Expected |
| --- | --- |
| No `Authorization` header | `401` with `WWW-Authenticate: Bearer error="missing_token"` |
| Valid token | `200`, with `sub` readable from the claims |
| Audience mismatch | `401 invalid_token` |
| Expired token | `401 invalid_token` |

Command:

```bash
bash "$SKILL_DIR/scripts/scaffold-resource-server.sh" --dir ./internal/auth --pkg auth
```

The generated tests cover all four cases; a failing `go test` fails verification.

## Behavior 3: Sign-out

| Observation | Expected |
| --- | --- |
| Visit `/logout` | The local session is cleared |
| Visit a protected page after sign-out | `401`; sign-in is required again |

yufu's central SSO session is independent of the application session: after signing out in the console,
`GET /api/v1/account/session` must go from `200` to `401`. In the loopback build, the `/oidc/logout`
confirmation page requires the legacy `refresh_token` cookie, so confirming a central session returns
`400 missing refresh token`; application sign-out must not depend on that page.

## Deployment-level verification

```bash
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

Three checks: issuer matches, JWKS is non-empty, and health is ready with a synchronized clock.

## Limits of automation

The sign-in and consent pages require human input, so a person must confirm the full browser flow for
behavior 1. The script verifies the most error-prone part, the client/parameter pairing. Sign-out works the same way.
