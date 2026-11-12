# yufu Runtime SDK for Go

`github.com/Bin-hy/yufu-sdk-go` is the Go reference implementation of the
yufu **Runtime SDK**. It is a thin, dependency-free HTTP/OIDC client for
applications that integrate with a yufu deployment.

> Status: v0.1 reference SDK. The module path is provisional while the SDK is
> developed inside the server repository; it will move to a dedicated
> `yufu-sdk-go` repository before a public release.

## Scope

Included:

- Discovery with issuer validation and caching
- Authorization Code + PKCE (S256), state and nonce generation
- Code exchange and refresh-token rotation with single-flight refresh
- UserInfo
- JWKS caching and RS256 verification
- Access-token verification (`typ=at+jwt`, `iss`, `aud`, `exp`, `nbf`)
- ID Token verification (`iss`, `aud`, `nonce`, `exp`)
- Introspection and revocation
- RP-initiated logout URL
- Back-channel logout token verification
- Typed OAuth/HTTP errors with `request_id` and `Retry-After`

Not included:

- M2M / `client_credentials`
- Permission synchronization, `/enforce`, legacy RBAC
- Management/console APIs (browser Cookie + CSRF)

## Install

```bash
go get github.com/Bin-hy/yufu-sdk-go
```

While the SDK lives in the server repository, use a `replace` directive:

```go
require github.com/Bin-hy/yufu-sdk-go v0.0.0
replace github.com/Bin-hy/yufu-sdk-go => /path/to/cobot-for/sdk/go
```

## Quickstart

```go
client, err := yufu.New(yufu.Config{
    Issuer:       "https://id.example.com",
    ClientID:     "web",
    ClientSecret: os.Getenv("OIDC_CLIENT_SECRET"),
    RedirectURI:  "https://app.example.com/callback",
    Scopes:       []string{"openid", "profile", "email"},
})
if err != nil {
    return err
}
defer client.Close()

auth, err := client.AuthorizeURL(yufu.AuthorizeOptions{})
if err != nil {
    return err
}
// Store auth.State, auth.Nonce and auth.CodeVerifier in the user's session,
// then redirect the browser to auth.URL.

tokens, err := client.ExchangeCode(ctx, code, auth.CodeVerifier)
if err != nil {
    return err
}
idClaims, err := client.VerifyIDToken(ctx, tokens.IDToken, auth.Nonce)
if err != nil {
    return err
}
accessClaims, err := client.VerifyAccessToken(ctx, tokens.AccessToken, "urn:app:production")
if err != nil {
    return err
}
profile, err := client.UserInfo(ctx, tokens.AccessToken)
```

The access-token audience is the application's registered access audience, not
the OIDC client id. A resource server must pass the expected audience to
`VerifyAccessToken`.

## Resource server

```go
reader, err := yufu.New(yufu.Config{
    Issuer:       "https://id.example.com",
    ClientID:     os.Getenv("READER_CLIENT_ID"),
    ClientSecret: os.Getenv("READER_CLIENT_SECRET"),
})
if err != nil {
    return err
}

claims, err := reader.VerifyAccessToken(ctx, bearerToken, "urn:app:production")
if err != nil {
    return err
}
introspection, err := reader.Introspect(ctx, bearerToken, yufu.TokenTypeAccessToken)
if err != nil {
    return err
}
if !introspection.Active || introspection.Subject != claims.Subject {
    return errors.New("token is not active or subject changed")
}
```

The SDK verifies the JWT locally. The introspection call is an additional
online check; the application decides its freshness policy. See
`docs/integration/enterprise-oidc.md` for the 2-second/5-second guidance used
by the reference examples.

## Back-channel logout

```go
claims, err := client.VerifyLogoutToken(ctx, logoutToken)
if err != nil {
    http.Error(w, err.Error(), http.StatusBadRequest)
    return
}
// Delete local sessions by claims.SID and claims.Subject.
// Persist claims.JTI to make delivery idempotent.
```

`VerifyLogoutToken` enforces `typ=logout+jwt`, `aud=client_id`, the
back-channel logout event, `jti`, `sid` or `sub`, no `nonce`, and a lifetime of
at most 120 seconds.

## Security defaults

- RS256 only; RSA keys smaller than 2048 bits are rejected.
- `kid` is required and JWKS is refreshed when an unknown `kid` appears.
- Access tokens must have `typ=at+jwt`; ID tokens must have `typ=JWT` or no
  `typ`; logout tokens must have `typ=logout+jwt`.
- `iss` must match the configured issuer exactly.
- Access-token `aud` is mandatory and must match the caller's expected
  audience.
- `exp`, `nbf` and `iat` are validated with a 60-second default leeway.
- HTTP responses are limited to 1 MiB; tokens are limited to 16 KiB.
- Discovery and JWKS caches default to 5 minutes.

## Testing

```bash
cd sdk/go
go test ./...
go vet ./...
```

The tests cover PKCE, code exchange, ID/access-token verification, JWKS
rotation, single-flight refresh, introspection, revocation, logout-token
validation and error mapping. `examples/sdk/go` contains an end-to-end example
and a mock-issuer test.
