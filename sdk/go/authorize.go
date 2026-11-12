package yufu

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"errors"
	"fmt"
	"net/url"
	"strings"
	"time"

	"github.com/Bin-hy/yufu-sdk-go/internal/pkce"
)

// AuthorizeOptions customizes an authorization request. Empty fields use the
// client configuration or safe generated values.
type AuthorizeOptions struct {
	State       string
	Nonce       string
	Scopes      []string
	RedirectURI string
	// Extra adds non-reserved authorization parameters.
	Extra url.Values
}

// AuthorizationRequest is the result of building an authorization URL. The
// caller must keep State, Nonce and CodeVerifier for the callback.
type AuthorizationRequest struct {
	URL           string
	State         string
	Nonce         string
	CodeVerifier  string
	CodeChallenge string
	RedirectURI   string
}

var reservedAuthorizeParams = map[string]bool{
	"client_id":             true,
	"redirect_uri":          true,
	"response_type":         true,
	"code_challenge":        true,
	"code_challenge_method": true,
	"scope":                 true,
	"state":                 true,
	"nonce":                 true,
}

// AuthorizeURL builds an authorization URL with a generated state, nonce and
// PKCE S256 verifier. It uses a 10s timeout for a discovery fetch if needed.
func (c *Client) AuthorizeURL(opts AuthorizeOptions) (*AuthorizationRequest, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	return c.AuthorizeURLContext(ctx, opts)
}

// AuthorizeURLContext is AuthorizeURL with a caller-supplied context.
func (c *Client) AuthorizeURLContext(ctx context.Context, opts AuthorizeOptions) (*AuthorizationRequest, error) {
	discovery, err := c.Discover(ctx)
	if err != nil {
		return nil, err
	}
	redirectURI := opts.RedirectURI
	if redirectURI == "" {
		redirectURI = c.cfg.RedirectURI
	}
	if redirectURI == "" {
		return nil, errors.New("yufu: RedirectURI is required for authorization")
	}
	scopes := opts.Scopes
	if len(scopes) == 0 {
		scopes = c.cfg.Scopes
	}
	if !containsString(scopes, "openid") {
		return nil, errors.New("yufu: scopes must include openid")
	}
	state := opts.State
	if state == "" {
		state, err = randomURLToken(32)
		if err != nil {
			return nil, err
		}
	}
	nonce := opts.Nonce
	if nonce == "" {
		nonce, err = randomURLToken(32)
		if err != nil {
			return nil, err
		}
	}
	verifier, err := pkce.NewVerifier()
	if err != nil {
		return nil, err
	}
	challenge := pkce.Challenge(verifier)

	values := url.Values{}
	for key, items := range opts.Extra {
		if reservedAuthorizeParams[key] {
			return nil, fmt.Errorf("yufu: authorization parameter %q is reserved", key)
		}
		for _, item := range items {
			values.Add(key, item)
		}
	}
	values.Set("client_id", c.cfg.ClientID)
	values.Set("redirect_uri", redirectURI)
	values.Set("response_type", "code")
	values.Set("code_challenge", challenge)
	values.Set("code_challenge_method", "S256")
	values.Set("scope", strings.Join(scopes, " "))
	values.Set("state", state)
	values.Set("nonce", nonce)

	endpoint, err := url.Parse(discovery.AuthorizationEndpoint)
	if err != nil {
		return nil, &Error{Code: "invalid_discovery", Description: "authorization_endpoint is invalid", Err: err}
	}
	endpoint.RawQuery = values.Encode()
	return &AuthorizationRequest{
		URL:           endpoint.String(),
		State:         state,
		Nonce:         nonce,
		CodeVerifier:  verifier,
		CodeChallenge: challenge,
		RedirectURI:   redirectURI,
	}, nil
}

// ExchangeCode redeems an authorization code with PKCE.
func (c *Client) ExchangeCode(ctx context.Context, code, codeVerifier string) (*TokenSet, error) {
	if strings.TrimSpace(code) == "" {
		return nil, errors.New("yufu: code is required")
	}
	if err := pkce.ValidateVerifier(codeVerifier); err != nil {
		return nil, err
	}
	if c.cfg.RedirectURI == "" {
		return nil, errors.New("yufu: RedirectURI is required for code exchange")
	}
	discovery, err := c.Discover(ctx)
	if err != nil {
		return nil, err
	}
	form := url.Values{}
	form.Set("grant_type", "authorization_code")
	form.Set("code", code)
	form.Set("code_verifier", codeVerifier)
	form.Set("redirect_uri", c.cfg.RedirectURI)
	auth := c.auth()
	c.applyAuth(form, auth)
	var response tokenResponse
	if err := c.doForm(ctx, discovery.TokenEndpoint, form, auth, &response); err != nil {
		return nil, err
	}
	return c.tokenSet(response)
}

func randomURLToken(size int) (string, error) {
	random := make([]byte, size)
	if _, err := rand.Read(random); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(random), nil
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
