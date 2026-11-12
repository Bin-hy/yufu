package yufu

import (
	"context"
	"fmt"
	"net/url"
)

// Discovery is the subset of OpenID Provider metadata the Runtime SDK uses.
type Discovery struct {
	Issuer                            string   `json:"issuer"`
	AuthorizationEndpoint             string   `json:"authorization_endpoint"`
	TokenEndpoint                     string   `json:"token_endpoint"`
	UserInfoEndpoint                  string   `json:"userinfo_endpoint"`
	JWKSURI                           string   `json:"jwks_uri"`
	RevocationEndpoint                string   `json:"revocation_endpoint"`
	IntrospectionEndpoint             string   `json:"introspection_endpoint"`
	EndSessionEndpoint                string   `json:"end_session_endpoint"`
	ResponseTypesSupported            []string `json:"response_types_supported"`
	ResponseModesSupported            []string `json:"response_modes_supported"`
	SubjectTypesSupported             []string `json:"subject_types_supported"`
	IDTokenSigningAlgValuesSupported  []string `json:"id_token_signing_alg_values_supported"`
	GrantTypesSupported               []string `json:"grant_types_supported"`
	TokenEndpointAuthMethodsSupported []string `json:"token_endpoint_auth_methods_supported"`
	IntrospectionAuthMethodsSupported []string `json:"introspection_endpoint_auth_methods_supported"`
	RevocationAuthMethodsSupported    []string `json:"revocation_endpoint_auth_methods_supported"`
	ScopesSupported                   []string `json:"scopes_supported"`
	ClaimsSupported                   []string `json:"claims_supported"`
	CodeChallengeMethodsSupported     []string `json:"code_challenge_methods_supported"`
	BackchannelLogoutSupported        bool     `json:"backchannel_logout_supported"`
	BackchannelLogoutSessionSupported bool     `json:"backchannel_logout_session_supported"`
}

// Discover fetches and caches the issuer's OpenID Provider metadata.
//
// The returned issuer must match Config.Issuer exactly after normalization.
func (c *Client) Discover(ctx context.Context) (*Discovery, error) {
	c.discoveryMu.Lock()
	defer c.discoveryMu.Unlock()
	if c.discovery != nil && c.now().Before(c.discoveryExpires) {
		return c.discovery, nil
	}
	var discovery Discovery
	endpoint := c.cfg.Issuer + "/.well-known/openid-configuration"
	if err := c.doGET(ctx, endpoint, &discovery); err != nil {
		return nil, err
	}
	if discovery.Issuer != c.cfg.Issuer {
		return nil, &Error{
			Code:        "invalid_discovery",
			Description: fmt.Sprintf("issuer mismatch: got %q want %q", discovery.Issuer, c.cfg.Issuer),
		}
	}
	for name, value := range map[string]string{
		"authorization_endpoint": discovery.AuthorizationEndpoint,
		"token_endpoint":         discovery.TokenEndpoint,
		"jwks_uri":               discovery.JWKSURI,
	} {
		if err := validateEndpoint(name, value); err != nil {
			return nil, err
		}
	}
	c.discovery = &discovery
	c.discoveryExpires = c.now().Add(c.cfg.DiscoveryTTL)
	return &discovery, nil
}

func validateEndpoint(name, value string) error {
	parsed, err := url.Parse(value)
	if err != nil || !parsed.IsAbs() || parsed.Host == "" ||
		(parsed.Scheme != "http" && parsed.Scheme != "https") {
		return &Error{
			Code:        "invalid_discovery",
			Description: fmt.Sprintf("%s is not an absolute http(s) URL", name),
		}
	}
	return nil
}
