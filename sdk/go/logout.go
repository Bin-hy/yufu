package cobotusr

import (
	"context"
	"errors"
	"net/url"
)

// EndSessionOptions configures an RP-initiated logout URL.
type EndSessionOptions struct {
	PostLogoutRedirectURI string
	IDTokenHint           string
	State                 string
}

// EndSessionURL builds the browser redirect for RP-initiated logout.
func (c *Client) EndSessionURL(ctx context.Context, opts EndSessionOptions) (string, error) {
	if opts.PostLogoutRedirectURI == "" {
		return "", errors.New("cobotusr: PostLogoutRedirectURI is required")
	}
	discovery, err := c.Discover(ctx)
	if err != nil {
		return "", err
	}
	if discovery.EndSessionEndpoint == "" {
		return "", &Error{Code: "unsupported", Description: "issuer does not publish end_session_endpoint"}
	}
	values := url.Values{}
	values.Set("client_id", c.cfg.ClientID)
	values.Set("post_logout_redirect_uri", opts.PostLogoutRedirectURI)
	if opts.IDTokenHint != "" {
		values.Set("id_token_hint", opts.IDTokenHint)
	}
	if opts.State != "" {
		values.Set("state", opts.State)
	}
	endpoint, err := url.Parse(discovery.EndSessionEndpoint)
	if err != nil {
		return "", &Error{Code: "invalid_discovery", Description: "end_session_endpoint is invalid", Err: err}
	}
	endpoint.RawQuery = values.Encode()
	return endpoint.String(), nil
}
