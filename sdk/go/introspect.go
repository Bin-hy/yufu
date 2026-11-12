package yufu

import (
	"context"
	"errors"
	"net/url"
	"strings"
	"time"

	"github.com/Bin-hy/yufu-sdk-go/internal/jose"
)

// TokenTypeHint is an optional RFC 7662 / RFC 7009 hint.
type TokenTypeHint string

const (
	TokenTypeAccessToken  TokenTypeHint = "access_token"
	TokenTypeRefreshToken TokenTypeHint = "refresh_token"
)

// Introspection is an RFC 7662 response. Inactive responses carry no metadata.
type Introspection struct {
	Active    bool
	Scope     string
	ClientID  string
	Username  string
	TokenType string
	ExpiresAt time.Time
	Subject   string
	SID       string
	Audience  string
	Issuer    string
	Raw       map[string]any
}

// Introspect calls the introspection endpoint with this client's credentials.
// Resource servers should construct a client with reader credentials.
func (c *Client) Introspect(ctx context.Context, token string, hint ...TokenTypeHint) (*Introspection, error) {
	if strings.TrimSpace(token) == "" {
		return nil, errors.New("yufu: token is required")
	}
	discovery, err := c.Discover(ctx)
	if err != nil {
		return nil, err
	}
	if discovery.IntrospectionEndpoint == "" {
		return nil, &Error{Code: "unsupported", Description: "issuer does not publish introspection_endpoint"}
	}
	form := url.Values{}
	form.Set("token", token)
	if len(hint) > 0 && hint[0] != "" {
		form.Set("token_type_hint", string(hint[0]))
	}
	auth := c.auth()
	c.applyAuth(form, auth)
	var raw map[string]any
	if err := c.doForm(ctx, discovery.IntrospectionEndpoint, form, auth, &raw); err != nil {
		return nil, err
	}
	return introspectionFrom(raw), nil
}

func introspectionFrom(raw map[string]any) *Introspection {
	active, _ := jose.BoolClaim(raw, "active")
	result := &Introspection{Active: active, Raw: raw}
	result.Scope, _ = jose.StringClaim(raw, "scope")
	result.ClientID, _ = jose.StringClaim(raw, "client_id")
	result.Username, _ = jose.StringClaim(raw, "username")
	result.TokenType, _ = jose.StringClaim(raw, "token_type")
	result.Subject, _ = jose.StringClaim(raw, "sub")
	result.SID, _ = jose.StringClaim(raw, "sid")
	result.Audience, _ = jose.StringClaim(raw, "aud")
	result.Issuer, _ = jose.StringClaim(raw, "iss")
	if exp, ok := jose.NumberClaim(raw, "exp"); ok {
		result.ExpiresAt = time.Unix(exp, 0).UTC()
	}
	return result
}
