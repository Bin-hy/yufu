package cobotusr

import (
	"context"
	"errors"
	"net/url"
	"strings"
)

// Revoke revokes an access or refresh token. Revocation is idempotent.
func (c *Client) Revoke(ctx context.Context, token string, hint ...TokenTypeHint) error {
	if strings.TrimSpace(token) == "" {
		return errors.New("cobotusr: token is required")
	}
	discovery, err := c.Discover(ctx)
	if err != nil {
		return err
	}
	if discovery.RevocationEndpoint == "" {
		return &Error{Code: "unsupported", Description: "issuer does not publish revocation_endpoint"}
	}
	form := url.Values{}
	form.Set("token", token)
	if len(hint) > 0 && hint[0] != "" {
		form.Set("token_type_hint", string(hint[0]))
	}
	auth := c.auth()
	c.applyAuth(form, auth)
	return c.doForm(ctx, discovery.RevocationEndpoint, form, auth, nil)
}
