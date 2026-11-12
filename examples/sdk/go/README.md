# Go Runtime SDK example

This example demonstrates the `yufu` Go Runtime SDK with a small web
application:

- `GET /login` builds an authorization URL with state, nonce and PKCE S256.
- `GET /callback` exchanges the code, verifies the ID Token and access token,
  and calls UserInfo.
- `GET /api/me` verifies a bearer access token and optionally introspects it
  with a Reader client.
- `GET /logout` builds an RP-initiated logout URL.
- `POST /oidc/backchannel-logout` verifies a logout token and acknowledges
  replayed `jti` values idempotently.

## Offline contract test (no server required)

```bash
cd examples/sdk/go
go test -count=1 ./...
```

The test starts a fake issuer, runs the full authorization-code flow through
the example handlers, and checks token verification, UserInfo, introspection,
logout URL construction and back-channel logout.

This test proves the SDK's protocol handling, **not** that it works against a
real `yufu` deployment. It is deliberately excluded from the live run:
`go test ./...` never touches the network.

## Live integration test (real yufu, real HTTP, real JWKS)

`integration_test.go` is guarded by the `integration` build tag and talks to a
real server: real login, real application registration, real authorization
code + PKCE exchange, real JWKS signature verification, real UserInfo, real
refresh and real revocation.

The easiest way to run it is the script. With no configuration it runs a
**self-contained local harness**: it starts Postgres, writes a loopback-only
config with `issuer = http://127.0.0.1:14000`, builds and starts `yufu`,
waits for discovery, runs the test and stops the server it started. The issuer
is not defaulted inside the SDK; the script writes it into the generated config
and exports the same value to the test, which then asserts that discovery
returns exactly that issuer.

```bash
cd examples/sdk/go
./run-integration.sh
```

To point the same test at an existing deployment, set `YUFU_ISSUER` (and
the admin credentials). In this mode the script does not start Postgres or a
local server:

```bash
cd examples/sdk/go
YUFU_ISSUER=https://id.example.com \
YUFU_ADMIN_USERNAME=admin \
YUFU_ADMIN_PASSWORD=... \
  ./run-integration.sh
```

To run the test directly against a server you already started:

```bash
cd examples/sdk/go
YUFU_ISSUER=http://127.0.0.1:14000 \
YUFU_ADMIN_USERNAME=sdk-admin \
YUFU_ADMIN_PASSWORD=sdk-admin-password \
  go test -tags=integration -run TestLive -v -count=1 ./...
```

Required variables:

| Variable | Meaning |
| --- | --- |
| `YUFU_ISSUER` | Base URL of the running server, e.g. `http://127.0.0.1:14000` |
| `YUFU_ADMIN_USERNAME` | Bootstrap super admin used to register a test application |
| `YUFU_ADMIN_PASSWORD` | Password for that admin |

Optional variables: `YUFU_REDIRECT_URI` (default
`http://127.0.0.1:18080/callback`), `YUFU_POST_LOGOUT_REDIRECT_URI`,
`YUFU_SCOPES` (default `openid profile email`), and
`YUFU_READER_CLIENT_ID` / `YUFU_READER_CLIENT_SECRET` to enable the
introspection assertion.

Missing required configuration is a hard failure. A skipped test would make a
green run indistinguishable from "never connected", which is exactly the
failure mode this test exists to prevent.

The live test registers a **public browser SPA client** and uses PKCE without a
client secret. That is intentional: confidential clients currently hit the
server's 2s security budget while verifying the bcrypt-hashed client secret and
return `503 deadline_exceeded`. The SDK supports `ClientSecret`, but the live
test does not depend on that server-side path.

## Run against a real yufu

Register a confidential web client and, optionally, an introspection Reader.
Then export:

```bash
export ISSUER=https://id.example.com
export CLIENT_ID=your-web-client
export CLIENT_SECRET=issued-secret
export REDIRECT_URI=http://127.0.0.1:8080/callback
export POST_LOGOUT_REDIRECT_URI=http://127.0.0.1:8080/
export AUDIENCE=urn:app:development
export SCOPES="openid profile email"

# Optional: enables the online introspection check in /api/me
export READER_CLIENT_ID=your-reader
export READER_CLIENT_SECRET=issued-reader-secret

go run .
```

Open <http://127.0.0.1:8080/> and sign in. The callback response shows the
verified ID Token, access-token and UserInfo claims; it never returns raw
tokens to the browser.

The example keeps pending authorization requests and seen logout `jti` values
in memory. A production application must use a durable session store and a
durable idempotency store.

Note: the interactive example above uses a confidential client. Until the
server-side bcrypt/security-budget issue is fixed, use a public SPA client with
PKCE (as the live integration test does) when running against a loopback
deployment.
