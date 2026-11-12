package cobotusr

import (
	"context"
	"crypto/rsa"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/Bin-hy/cobot-sdk-go/internal/jose"
)

const backchannelLogoutEvent = "http://schemas.openid.net/event/backchannel-logout"

type verifyOptions struct {
	typ             []string
	audience        string
	requireAudience bool
}

// VerifyAccessToken verifies an at+jwt access token for a resource server.
// audience is the application access audience, not the OIDC client id.
func (c *Client) VerifyAccessToken(ctx context.Context, token, audience string) (*AccessTokenClaims, error) {
	if strings.TrimSpace(audience) == "" {
		return nil, errors.New("cobotusr: audience is required")
	}
	parsed, err := c.verifyJWT(ctx, token, verifyOptions{
		typ:             []string{"at+jwt"},
		audience:        audience,
		requireAudience: true,
	})
	if err != nil {
		return nil, err
	}
	return accessTokenClaimsFrom(parsed.Claims)
}

// VerifyIDToken verifies an ID Token for Config.ClientID. When nonce is
// non-empty it must match the nonce from the authorization request.
func (c *Client) VerifyIDToken(ctx context.Context, token, nonce string) (*IDTokenClaims, error) {
	parsed, err := c.verifyJWT(ctx, token, verifyOptions{
		typ:             []string{"JWT", ""},
		audience:        c.cfg.ClientID,
		requireAudience: true,
	})
	if err != nil {
		return nil, err
	}
	if nonce != "" {
		actual, ok := jose.StringClaim(parsed.Claims, "nonce")
		if !ok || actual != nonce {
			return nil, &Error{Code: "invalid_token", Description: "ID token nonce does not match"}
		}
	}
	return idTokenClaimsFrom(parsed.Claims)
}

// VerifyLogoutToken verifies an OIDC back-channel logout token for
// Config.ClientID and enforces the cobotusr logout-token profile.
func (c *Client) VerifyLogoutToken(ctx context.Context, token string) (*LogoutTokenClaims, error) {
	parsed, err := c.verifyJWT(ctx, token, verifyOptions{
		typ:             []string{"logout+jwt"},
		audience:        c.cfg.ClientID,
		requireAudience: true,
	})
	if err != nil {
		return nil, err
	}
	if _, exists := parsed.Claims["nonce"]; exists {
		return nil, &Error{Code: "invalid_token", Description: "logout token must not contain nonce"}
	}
	jti, ok := jose.StringClaim(parsed.Claims, "jti")
	if !ok || jti == "" {
		return nil, &Error{Code: "invalid_token", Description: "logout token is missing jti"}
	}
	issuedAt, ok := jose.NumberClaim(parsed.Claims, "iat")
	if !ok {
		return nil, &Error{Code: "invalid_token", Description: "logout token is missing iat"}
	}
	expiresAt, ok := jose.NumberClaim(parsed.Claims, "exp")
	if !ok {
		return nil, &Error{Code: "invalid_token", Description: "logout token is missing exp"}
	}
	if expiresAt <= issuedAt || expiresAt-issuedAt > 120 {
		return nil, &Error{Code: "invalid_token", Description: "logout token lifetime must be at most 120 seconds"}
	}
	subject, _ := jose.StringClaim(parsed.Claims, "sub")
	sid, _ := jose.StringClaim(parsed.Claims, "sid")
	if subject == "" && sid == "" {
		return nil, &Error{Code: "invalid_token", Description: "logout token requires sub or sid"}
	}
	eventsValue, ok := parsed.Claims["events"]
	if !ok {
		return nil, &Error{Code: "invalid_token", Description: "logout token is missing events"}
	}
	events, ok := eventsValue.(map[string]any)
	if !ok || len(events) != 1 {
		return nil, &Error{Code: "invalid_token", Description: "logout token must contain exactly one event"}
	}
	eventValue, ok := events[backchannelLogoutEvent]
	if !ok {
		return nil, &Error{Code: "invalid_token", Description: "logout token has an unexpected event"}
	}
	event, ok := eventValue.(map[string]any)
	if !ok || len(event) != 0 {
		return nil, &Error{Code: "invalid_token", Description: "back-channel logout event must be an empty object"}
	}
	audiences, err := jose.AudienceClaim(parsed.Claims)
	if err != nil {
		return nil, &Error{Code: "invalid_token", Description: err.Error(), Err: err}
	}
	return &LogoutTokenClaims{
		Issuer:    c.cfg.Issuer,
		Audience:  audiences[0],
		Subject:   subject,
		SID:       sid,
		JTI:       jti,
		IssuedAt:  time.Unix(issuedAt, 0).UTC(),
		ExpiresAt: time.Unix(expiresAt, 0).UTC(),
		Events:    events,
		Raw:       parsed.Claims,
	}, nil
}

func (c *Client) verifyJWT(ctx context.Context, token string, opts verifyOptions) (*jose.Token, error) {
	parsed, err := jose.Parse(token)
	if err != nil {
		return nil, tokenError(err)
	}
	if len(opts.typ) > 0 && !matchesAny(parsed.Header.Typ, opts.typ) {
		return nil, &Error{
			Code:        "invalid_token",
			Description: fmt.Sprintf("unexpected token typ %q", parsed.Header.Typ),
		}
	}
	key, err := c.keyFor(ctx, parsed.Header.Kid)
	if err != nil {
		return nil, err
	}
	if err := parsed.Verify(key); err != nil {
		return nil, &Error{Code: "invalid_token", Description: "signature verification failed", Err: err}
	}
	issuer, ok := jose.StringClaim(parsed.Claims, "iss")
	if !ok || issuer != c.cfg.Issuer {
		return nil, &Error{Code: "invalid_token", Description: "issuer does not match"}
	}
	if opts.requireAudience {
		audiences, err := jose.AudienceClaim(parsed.Claims)
		if err != nil || !containsString(audiences, opts.audience) {
			return nil, &Error{Code: "invalid_token", Description: "audience does not match"}
		}
	}
	now := c.now()
	expiresAt, ok := jose.NumberClaim(parsed.Claims, "exp")
	if !ok {
		return nil, &Error{Code: "invalid_token", Description: "token is missing exp"}
	}
	if now.After(time.Unix(expiresAt, 0).Add(c.cfg.Leeway)) {
		return nil, &Error{Code: "invalid_token", Description: "token is expired"}
	}
	if notBefore, ok := jose.NumberClaim(parsed.Claims, "nbf"); ok {
		if now.Add(c.cfg.Leeway).Before(time.Unix(notBefore, 0)) {
			return nil, &Error{Code: "invalid_token", Description: "token is not valid yet"}
		}
	}
	if issuedAt, ok := jose.NumberClaim(parsed.Claims, "iat"); ok {
		if time.Unix(issuedAt, 0).After(now.Add(c.cfg.Leeway)) {
			return nil, &Error{Code: "invalid_token", Description: "token was issued in the future"}
		}
	}
	return parsed, nil
}

func (c *Client) keyFor(ctx context.Context, kid string) (*rsa.PublicKey, error) {
	c.jwksMu.Lock()
	defer c.jwksMu.Unlock()
	now := c.now()
	if c.jwks != nil && now.Before(c.jwksExpires) {
		if key, ok := c.jwks[kid]; ok {
			return key, nil
		}
	}
	keys, err := c.fetchJWKS(ctx)
	if err != nil {
		return nil, err
	}
	c.jwks = keys
	c.jwksExpires = now.Add(c.cfg.JWKSTTL)
	if key, ok := keys[kid]; ok {
		return key, nil
	}
	return nil, &Error{
		Code:        "invalid_token",
		Description: fmt.Sprintf("JWKS has no key for kid %q", kid),
		Err:         jose.ErrKeyNotFound,
	}
}

func (c *Client) fetchJWKS(ctx context.Context) (map[string]*rsa.PublicKey, error) {
	discovery, err := c.Discover(ctx)
	if err != nil {
		return nil, err
	}
	var raw json.RawMessage
	if err := c.doGET(ctx, discovery.JWKSURI, &raw); err != nil {
		return nil, err
	}
	parsed, err := jose.ParseJWKS(raw)
	if err != nil {
		return nil, &Error{Code: "invalid_jwks", Description: err.Error(), Err: err}
	}
	keys := make(map[string]*rsa.PublicKey, len(parsed))
	for _, key := range parsed {
		keys[key.Kid] = key.PublicKey
	}
	return keys, nil
}

func tokenError(err error) *Error {
	code := "invalid_token"
	if errors.Is(err, jose.ErrUnsupportedAlgorithm) {
		code = "unsupported_algorithm"
	}
	return &Error{Code: code, Description: err.Error(), Err: err}
}

func matchesAny(value string, expected []string) bool {
	for _, candidate := range expected {
		if strings.EqualFold(value, candidate) {
			return true
		}
	}
	return false
}
