package yufu

import (
	"context"
	"errors"
	"strings"

	"github.com/Bin-hy/yufu-sdk-go/internal/jose"
)

// UserInfo is the profile returned by the UserInfo endpoint.
type UserInfo struct {
	Subject           string
	PreferredUsername string
	Name              string
	Nickname          string
	Email             string
	Raw               map[string]any
}

// UserInfo fetches the profile for an access token.
func (c *Client) UserInfo(ctx context.Context, accessToken string) (*UserInfo, error) {
	if strings.TrimSpace(accessToken) == "" {
		return nil, errors.New("yufu: access token is required")
	}
	discovery, err := c.Discover(ctx)
	if err != nil {
		return nil, err
	}
	if discovery.UserInfoEndpoint == "" {
		return nil, &Error{Code: "unsupported", Description: "issuer does not publish userinfo_endpoint"}
	}
	var raw map[string]any
	if err := c.doGETWithBearer(ctx, discovery.UserInfoEndpoint, accessToken, &raw); err != nil {
		return nil, err
	}
	subject, ok := jose.StringClaim(raw, "sub")
	if !ok || subject == "" {
		return nil, &Error{Code: "invalid_response", Description: "userinfo is missing sub"}
	}
	info := &UserInfo{Subject: subject, Raw: raw}
	info.PreferredUsername, _ = jose.StringClaim(raw, "preferred_username")
	info.Name, _ = jose.StringClaim(raw, "name")
	info.Nickname, _ = jose.StringClaim(raw, "nickname")
	info.Email, _ = jose.StringClaim(raw, "email")
	return info, nil
}
