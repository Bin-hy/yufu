package cobotusr

import (
	"context"
	"crypto/rsa"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"
)

const (
	defaultDiscoveryTTL = 5 * time.Minute
	defaultJWKSTTL      = 5 * time.Minute
	defaultLeeway       = 60 * time.Second
	maxResponseSize     = 1 << 20
)

// TokenAuthMethod is the client authentication method used at the token,
// introspection and revocation endpoints.
type TokenAuthMethod string

const (
	// TokenAuthPost sends client_id and client_secret in the form body.
	TokenAuthPost TokenAuthMethod = "client_secret_post"
	// TokenAuthBasic sends client credentials with HTTP Basic authentication.
	TokenAuthBasic TokenAuthMethod = "client_secret_basic"
	// TokenAuthNone is used by public clients without a secret.
	TokenAuthNone TokenAuthMethod = "none"
)

// Config configures a Runtime SDK client.
//
// Issuer and ClientID are required. ClientSecret is optional for public
// clients. RedirectURI is required only for AuthorizeURL and ExchangeCode.
type Config struct {
	Issuer       string
	ClientID     string
	ClientSecret string
	RedirectURI  string
	Scopes       []string

	// TokenAuthMethod defaults to client_secret_post when ClientSecret is set,
	// otherwise none.
	TokenAuthMethod TokenAuthMethod

	// HTTPClient is optional. When nil the SDK creates a client with a 10s
	// timeout and closes its idle connections on Close.
	HTTPClient *http.Client
	UserAgent  string

	// DiscoveryTTL and JWKSTTL control in-process caches. Defaults are 5m.
	DiscoveryTTL time.Duration
	JWKSTTL      time.Duration

	// Leeway is applied to exp, nbf and iat validation. Defaults to 60s.
	Leeway time.Duration

	// Now is injectable for deterministic tests. Defaults to time.Now.
	Now func() time.Time
}

// Client is a concurrency-safe Runtime SDK client.
type Client struct {
	cfg            Config
	httpClient     *http.Client
	ownsHTTPClient bool
	now            func() time.Time

	discoveryMu      sync.Mutex
	discovery        *Discovery
	discoveryExpires time.Time

	jwksMu      sync.Mutex
	jwks        map[string]*rsa.PublicKey
	jwksExpires time.Time

	refreshMu sync.Mutex
	refreshes map[string]*refreshCall
}

type refreshCall struct {
	done   chan struct{}
	tokens *TokenSet
	err    error
}

type requestAuth struct {
	method       TokenAuthMethod
	clientID     string
	clientSecret string
}

// New validates cfg and returns a Runtime SDK client.
func New(cfg Config) (*Client, error) {
	issuer, err := normalizeIssuer(cfg.Issuer)
	if err != nil {
		return nil, err
	}
	cfg.Issuer = issuer
	if strings.TrimSpace(cfg.ClientID) == "" {
		return nil, errors.New("cobotusr: ClientID is required")
	}
	if len(cfg.Scopes) == 0 {
		cfg.Scopes = []string{"openid"}
	}
	if cfg.TokenAuthMethod == "" {
		if cfg.ClientSecret != "" {
			cfg.TokenAuthMethod = TokenAuthPost
		} else {
			cfg.TokenAuthMethod = TokenAuthNone
		}
	}
	switch cfg.TokenAuthMethod {
	case TokenAuthPost, TokenAuthBasic:
		if cfg.ClientSecret == "" {
			return nil, fmt.Errorf("cobotusr: ClientSecret is required for %s", cfg.TokenAuthMethod)
		}
	case TokenAuthNone:
		if cfg.ClientSecret != "" {
			return nil, errors.New("cobotusr: ClientSecret must be empty for TokenAuthNone")
		}
	default:
		return nil, fmt.Errorf("cobotusr: unsupported TokenAuthMethod %q", cfg.TokenAuthMethod)
	}
	if cfg.DiscoveryTTL <= 0 {
		cfg.DiscoveryTTL = defaultDiscoveryTTL
	}
	if cfg.JWKSTTL <= 0 {
		cfg.JWKSTTL = defaultJWKSTTL
	}
	if cfg.Leeway < 0 {
		return nil, errors.New("cobotusr: Leeway must not be negative")
	}
	if cfg.Leeway == 0 {
		cfg.Leeway = defaultLeeway
	}
	if cfg.UserAgent == "" {
		cfg.UserAgent = "cobotusr-go-sdk/" + Version
	}
	ownsHTTPClient := false
	if cfg.HTTPClient == nil {
		cfg.HTTPClient = &http.Client{Timeout: 10 * time.Second}
		ownsHTTPClient = true
	}
	now := cfg.Now
	if now == nil {
		now = time.Now
	}
	return &Client{
		cfg:            cfg,
		httpClient:     cfg.HTTPClient,
		ownsHTTPClient: ownsHTTPClient,
		now:            now,
		refreshes:      make(map[string]*refreshCall),
	}, nil
}

// Close releases idle HTTP connections owned by the SDK.
func (c *Client) Close() error {
	if c.ownsHTTPClient {
		c.httpClient.CloseIdleConnections()
	}
	return nil
}

func normalizeIssuer(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", errors.New("cobotusr: Issuer is required")
	}
	parsed, err := url.Parse(raw)
	if err != nil || !parsed.IsAbs() || parsed.Host == "" ||
		(parsed.Scheme != "http" && parsed.Scheme != "https") {
		return "", errors.New("cobotusr: Issuer must be an absolute http(s) URL")
	}
	if parsed.RawQuery != "" || parsed.Fragment != "" {
		return "", errors.New("cobotusr: Issuer must not contain a query or fragment")
	}
	parsed.Path = strings.TrimRight(parsed.Path, "/")
	return parsed.String(), nil
}

func (c *Client) auth() requestAuth {
	return requestAuth{
		method:       c.cfg.TokenAuthMethod,
		clientID:     c.cfg.ClientID,
		clientSecret: c.cfg.ClientSecret,
	}
}

func (c *Client) applyAuth(form url.Values, auth requestAuth) {
	switch auth.method {
	case TokenAuthPost:
		form.Set("client_id", auth.clientID)
		form.Set("client_secret", auth.clientSecret)
	case TokenAuthNone:
		form.Set("client_id", auth.clientID)
	}
}

func (c *Client) doForm(ctx context.Context, endpoint string, form url.Values, auth requestAuth, out any) error {
	request, err := http.NewRequestWithContext(
		ctx,
		http.MethodPost,
		endpoint,
		strings.NewReader(form.Encode()),
	)
	if err != nil {
		return &Error{Code: "invalid_request", Err: err}
	}
	request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	if auth.method == TokenAuthBasic {
		request.SetBasicAuth(auth.clientID, auth.clientSecret)
	}
	return c.do(request, out)
}

func (c *Client) doGET(ctx context.Context, endpoint string, out any) error {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return &Error{Code: "invalid_request", Err: err}
	}
	return c.do(request, out)
}

func (c *Client) doGETWithBearer(ctx context.Context, endpoint, token string, out any) error {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return &Error{Code: "invalid_request", Err: err}
	}
	request.Header.Set("Authorization", "Bearer "+token)
	return c.do(request, out)
}

func (c *Client) do(request *http.Request, out any) error {
	request.Header.Set("Accept", "application/json")
	request.Header.Set("User-Agent", c.cfg.UserAgent)
	response, err := c.httpClient.Do(request)
	if err != nil {
		return &Error{Code: "network_error", Err: err}
	}
	defer response.Body.Close()
	body, err := readLimited(response.Body, maxResponseSize)
	if err != nil {
		return &Error{StatusCode: response.StatusCode, Code: "network_error", Err: err}
	}
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return newHTTPError(response, body)
	}
	if out == nil || len(body) == 0 {
		return nil
	}
	if err := json.Unmarshal(body, out); err != nil {
		return &Error{
			StatusCode:  response.StatusCode,
			Code:        "invalid_response",
			Description: err.Error(),
			Body:        body,
			Err:         err,
		}
	}
	return nil
}

func readLimited(reader io.Reader, limit int64) ([]byte, error) {
	data, err := io.ReadAll(io.LimitReader(reader, limit+1))
	if err != nil {
		return nil, err
	}
	if int64(len(data)) > limit {
		return nil, errors.New("response body exceeds the SDK limit")
	}
	return data, nil
}
