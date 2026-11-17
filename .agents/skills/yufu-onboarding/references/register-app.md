# Application Registration

Registration goes through the console admin API; no browser interaction is needed. It relies on the
bootstrap admin account written by the local deployment.

## Modes

| Mode | `client_kind` | Secret | Use for |
| --- | --- | --- | --- |
| `web` | `server_web` | Yes | Web backends that keep the session on the server |
| `api` | `browser_spa` | No | Resource servers that only verify Bearer tokens |

`server_web` must not set `browser_origins`; for `browser_spa`, `browser_origins` must exactly match the
origins of all redirect URIs. The script handles this per mode (it deduplicates the sign-out URL when it
shares an origin). Do not mix these values by hand.

## Commands

```bash
bash "$SKILL_DIR/scripts/register-app.sh" --mode web \
  --redirect-uri http://127.0.0.1:8080/callback \
  --post-logout-uri http://127.0.0.1:8080/ \
  --audience urn:app:development \
  --scopes "openid profile email"
```

```bash
bash "$SKILL_DIR/scripts/register-app.sh" --mode api --audience urn:app:development
```

Defaults are defined in the script; `--app-key` is derived from the project path by default, so reruns in
the same project stay stable.

## Call chain

1. `GET /api/v1/identity/csrf` with `Origin`; store the `csrf_token` and the cookies.
2. `POST /api/v1/identity/login` with `Origin`, `x-csrf-token`, and the username/password.
3. `POST /api/v1/admin/apps` with `Origin`, `x-csrf-token`, and `idempotency-key`.
4. Read the development environment's `client_id` from the response; confidential clients also receive a one-time `client_secret`.
5. Write `state.env` and `client_secret` (mode 600).

## Idempotency

- The idempotency key is `app_key`. Reruns in the same project hit the same operation receipt.
- A replayed request returns only the application and client id; **it never returns the secret again**.
- When `state.env` already has `YUFU_CLIENT_ID`, the script goes straight to the parameter check and does
  not create a duplicate application.
- If the secret file is missing, the script fails explicitly: reset the secret in the console, or register
  again with a new `--app-key`.

## Parameter semantics

| Parameter | Constraint |
| --- | --- |
| `redirect_uri` | Byte-for-byte identical to the authorization request, including scheme, host, port, and path; no wildcards |
| `post_logout_redirect_uri` | Same rules; the loopback build requires it to also appear in `redirect_uris`, and the script adds it automatically |
| `access_audience` | Audience used by resource servers; not the client id |
| `identity_scopes` | Space-separated; `openid` is required; commonly `openid profile email` |
| `consent_policy` | Always `user_consent` in this project |
| `logout_capability` | Always `online`; `backchannel_and_online` also requires the callback address |

## Failure reference

| HTTP | Meaning | Action |
| --- | --- | --- |
| `401` | Login failed or cookies did not take effect | Check the admin account and `allow_insecure_loopback_cookies` |
| `403` | The account lacks admin permission | Use the bootstrap admin from deployment |
| `400 invalid_input` | Invalid parameter combination | Re-check the constraints above; the most common `server_web` cause is a stray `browser_origins` |
| `409` | `app_key` exists with a different idempotency key | Reuse the original `--app-key`, or use a new key |
