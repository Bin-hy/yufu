# Go Resource Server Integration

Use this when the application already has a sign-in system and only needs to verify yufu-issued access
tokens on its own API.

## 1. Get the three values first

| Value | Source |
| --- | --- |
| `issuer` | `YUFU_ISSUER` in `state.env`, for example `http://127.0.0.1:14000` |
| `client_id` | `YUFU_CLIENT_ID` in `state.env` (only used to initialize the SDK) |
| `audience` | `YUFU_AUDIENCE` in `state.env`, for example `urn:app:development` |

`audience` is the environment's access audience, **not** the client id. Token verification depends only on
the issuer and JWKS; `client_id` is required by SDK initialization and does not participate in signature
verification.

## 2. Generate the middleware

```bash
bash "$SKILL_DIR/scripts/scaffold-resource-server.sh" --dir ./internal/auth --pkg auth
```

The script does three things: render `yufu_auth.go` and `yufu_auth_test.go`, pin the SDK to the version in
`assets/versions.env` (override with `YUFU_SDK_VERSION`), and run `go test`. The target directory must be
inside an initialized Go module.

Generated files:

| File | Contents |
| --- | --- |
| `yufu_auth.go` | `Verifier`, `NewVerifier`, `Middleware`, `ClaimsFromContext` |
| `yufu_auth_test.go` | Four cases: valid token, no token, audience mismatch, expired token |

## 3. Wire it in

```go
client, err := yufu.New(yufu.Config{
	Issuer:   issuer,
	ClientID: clientID,
})
if err != nil {
	return err
}
defer client.Close()

verifier, err := auth.NewVerifier(client, audience)
if err != nil {
	return err
}

mux.Handle("/api/", verifier.Middleware(apiHandler))
```

Read claims in the handler:

```go
claims, ok := auth.ClaimsFromContext(r.Context())
if !ok {
	http.Error(w, "unauthorized", http.StatusUnauthorized)
	return
}
// claims.Subject is the stable subject; key local users by iss + sub, not email.
```

## 4. Error contract

| Case | Response |
| --- | --- |
| No `Authorization` header | `401` + `WWW-Authenticate: Bearer error="missing_token"` |
| Signature / issuer / audience / expiry mismatch | `401` + `WWW-Authenticate: Bearer error="invalid_token"` |

Never echo the token or the verifier's internal error text in a 401 response.

## 5. When introspection is needed

Local verification already covers signature, issuer, audience, `typ`, and expiry, so most requests need no
extra network call. Call `Introspect` only when you need the immediate semantics of "has this token been
revoked actively?", and then:

- Register a Reader credential and keep it server-side only.
- Evict cache entries by `sid` and `sub`.
- Never replace local verification with introspection.

## 6. Relationship to the script

`scaffold-resource-server.sh` generates the middleware described in this section. Rerunning it overwrites
files with the same names, so keep project-specific logic in the caller rather than editing the generated files.
