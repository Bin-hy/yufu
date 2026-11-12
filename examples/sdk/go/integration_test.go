//go:build integration

// Live integration test for the Go Runtime SDK.
//
// Unlike sdk_test.go (which runs against an in-process fake issuer), this file
// talks to a real cobotusr deployment: real discovery, real JWKS, real login,
// real application registration, real authorization code exchange and real
// revocation.
//
// Run it with ./run-integration.sh, or against an already running server:
//
//	COBOTUSR_ISSUER=http://127.0.0.1:14000 \
//	COBOTUSR_ADMIN_USERNAME=sdk-admin \
//	COBOTUSR_ADMIN_PASSWORD=sdk-admin-password \
//	go test -tags=integration -run TestLive -v ./...
//
// Missing configuration is a hard failure on purpose: a green `go test ./...`
// must never be mistaken for proof that the SDK works against a real server.
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/cookiejar"
	"net/url"
	"os"
	"strings"
	"testing"
	"time"

	cobotusr "github.com/Bin-hy/cobot-sdk-go"
)

const liveAudience = "urn:cobotusr:sdk-integration"

type liveConfig struct {
	issuer                string
	adminUsername         string
	adminPassword         string
	redirectURI           string
	postLogoutRedirectURI string
	scopes                []string
	readerClientID        string
	readerClientSecret    string
}

func loadLiveConfig(t *testing.T) liveConfig {
	t.Helper()
	required := []string{
		"COBOTUSR_ISSUER",
		"COBOTUSR_ADMIN_USERNAME",
		"COBOTUSR_ADMIN_PASSWORD",
	}
	var missing []string
	for _, name := range required {
		if strings.TrimSpace(os.Getenv(name)) == "" {
			missing = append(missing, name)
		}
	}
	if len(missing) > 0 {
		t.Fatalf("live integration test is enabled but not configured; missing: %s\n"+
			"run ./run-integration.sh, or export the variables documented in README.md",
			strings.Join(missing, ", "))
	}

	cfg := liveConfig{
		issuer:             strings.TrimSuffix(os.Getenv("COBOTUSR_ISSUER"), "/"),
		adminUsername:      os.Getenv("COBOTUSR_ADMIN_USERNAME"),
		adminPassword:      os.Getenv("COBOTUSR_ADMIN_PASSWORD"),
		redirectURI:        os.Getenv("COBOTUSR_REDIRECT_URI"),
		readerClientID:     os.Getenv("COBOTUSR_READER_CLIENT_ID"),
		readerClientSecret: os.Getenv("COBOTUSR_READER_CLIENT_SECRET"),
	}
	if cfg.redirectURI == "" {
		cfg.redirectURI = "http://127.0.0.1:18080/callback"
	}
	cfg.postLogoutRedirectURI = os.Getenv("COBOTUSR_POST_LOGOUT_REDIRECT_URI")
	if cfg.postLogoutRedirectURI == "" {
		cfg.postLogoutRedirectURI = strings.TrimSuffix(cfg.redirectURI, "/callback") + "/"
	}
	scopes := os.Getenv("COBOTUSR_SCOPES")
	if scopes == "" {
		scopes = "openid profile email"
	}
	cfg.scopes = strings.Fields(scopes)
	if (cfg.readerClientID == "") != (cfg.readerClientSecret == "") {
		t.Fatal("COBOTUSR_READER_CLIENT_ID and COBOTUSR_READER_CLIENT_SECRET must be set together")
	}
	return cfg
}

func TestLiveAuthorizationCodeFlow(t *testing.T) {
	cfg := loadLiveConfig(t)
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	browser := newLiveBrowser(t, cfg.issuer)
	session := browser.login(ctx, cfg.adminUsername, cfg.adminPassword)
	t.Logf("logged in as %s (user_id %d, administrator=%v)", session.Username, session.UserID, session.Administrator)

	clientID := browser.registerPublicApp(ctx, session.UserID, cfg)
	t.Logf("registered live application client_id=%s", clientID)

	client, err := cobotusr.New(cobotusr.Config{
		Issuer:      cfg.issuer,
		ClientID:    clientID,
		RedirectURI: cfg.redirectURI,
		Scopes:      cfg.scopes,
	})
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	defer client.Close()

	discovery, err := client.Discover(ctx)
	if err != nil {
		t.Fatalf("discovery against %s: %v", cfg.issuer, err)
	}
	if discovery.Issuer != cfg.issuer {
		t.Fatalf("discovery issuer = %q, want %q", discovery.Issuer, cfg.issuer)
	}
	if discovery.TokenEndpoint == "" || discovery.JWKSURI == "" || discovery.UserInfoEndpoint == "" {
		t.Fatalf("discovery is missing required endpoints: %+v", discovery)
	}
	if !discovery.BackchannelLogoutSupported {
		t.Error("discovery does not advertise backchannel_logout_supported")
	}
	if !liveContains(discovery.CodeChallengeMethodsSupported, "S256") {
		t.Errorf("discovery does not advertise S256 PKCE: %v", discovery.CodeChallengeMethodsSupported)
	}
	if !liveContains(discovery.GrantTypesSupported, "authorization_code") {
		t.Errorf("discovery does not advertise authorization_code: %v", discovery.GrantTypesSupported)
	}
	t.Logf("discovered %s (jwks %s)", discovery.Issuer, discovery.JWKSURI)

	authorize, err := client.AuthorizeURLContext(ctx, cobotusr.AuthorizeOptions{})
	if err != nil {
		t.Fatalf("authorize url: %v", err)
	}

	redirect := browser.authorize(ctx, authorize.URL)
	if flowID := liveConsentFlowID(redirect); flowID != "" {
		t.Logf("consent required for flow %s", flowID)
		redirect = browser.approveConsent(ctx, flowID)
	}
	if code := redirect.Query().Get("error"); code != "" {
		t.Fatalf("authorization failed: %s (%s)", code, redirect.Query().Get("error_description"))
	}
	if got := redirect.Query().Get("state"); got != authorize.State {
		t.Fatalf("authorization state = %q, want %q", got, authorize.State)
	}
	code := redirect.Query().Get("code")
	if code == "" {
		t.Fatalf("authorization redirect has no code: %s", redirect)
	}

	tokens, err := client.ExchangeCode(ctx, code, authorize.CodeVerifier)
	if err != nil {
		t.Fatalf("exchange code: %v", err)
	}
	if tokens.AccessToken == "" || tokens.IDToken == "" || tokens.RefreshToken == "" {
		t.Fatalf("token response is incomplete: %+v", tokens)
	}

	idClaims, err := client.VerifyIDToken(ctx, tokens.IDToken, authorize.Nonce)
	if err != nil {
		t.Fatalf("verify id token: %v", err)
	}
	if idClaims.Audience != clientID {
		t.Fatalf("id token aud = %q, want %q", idClaims.Audience, clientID)
	}
	if idClaims.Subject == "" {
		t.Fatal("id token has no sub")
	}
	if _, err := client.VerifyIDToken(ctx, tokens.IDToken, "wrong-nonce"); err == nil {
		t.Fatal("id token verified with a wrong nonce")
	}

	accessClaims, err := client.VerifyAccessToken(ctx, tokens.AccessToken, liveAudience)
	if err != nil {
		t.Fatalf("verify access token: %v", err)
	}
	if accessClaims.Subject != idClaims.Subject {
		t.Fatalf("access token sub = %q, id token sub = %q", accessClaims.Subject, idClaims.Subject)
	}
	if accessClaims.TokenUse != "user_session" {
		t.Errorf("access token token_use = %q, want user_session", accessClaims.TokenUse)
	}
	if _, err := client.VerifyAccessToken(ctx, tokens.AccessToken, "urn:cobotusr:wrong-audience"); err == nil {
		t.Fatal("access token verified with a wrong audience")
	}
	if _, err := client.VerifyLogoutToken(ctx, tokens.AccessToken); err == nil {
		t.Fatal("access token was accepted as a logout token")
	}

	userinfo, err := client.UserInfo(ctx, tokens.AccessToken)
	if err != nil {
		t.Fatalf("userinfo: %v", err)
	}
	if userinfo.Subject != idClaims.Subject {
		t.Fatalf("userinfo sub = %q, id token sub = %q", userinfo.Subject, idClaims.Subject)
	}
	if userinfo.PreferredUsername != cfg.adminUsername {
		t.Errorf("userinfo preferred_username = %q, want %q", userinfo.PreferredUsername, cfg.adminUsername)
	}

	refreshed, err := client.Refresh(ctx, tokens.RefreshToken)
	if err != nil {
		t.Fatalf("refresh: %v", err)
	}
	if refreshed.AccessToken == tokens.AccessToken {
		t.Error("refresh returned the same access token")
	}
	if _, err := client.VerifyAccessToken(ctx, refreshed.AccessToken, liveAudience); err != nil {
		t.Fatalf("verify refreshed access token: %v", err)
	}

	if cfg.readerClientID != "" {
		reader, err := cobotusr.New(cobotusr.Config{
			Issuer:       cfg.issuer,
			ClientID:     cfg.readerClientID,
			ClientSecret: cfg.readerClientSecret,
		})
		if err != nil {
			t.Fatalf("new reader client: %v", err)
		}
		defer reader.Close()
		introspection, err := reader.Introspect(ctx, tokens.AccessToken, cobotusr.TokenTypeAccessToken)
		if err != nil {
			t.Fatalf("introspect: %v", err)
		}
		if !introspection.Active {
			t.Fatalf("introspection reports the live token as inactive: %+v", introspection)
		}
		if introspection.ClientID != clientID {
			t.Errorf("introspection client_id = %q, want %q", introspection.ClientID, clientID)
		}
	}

	if err := client.Revoke(ctx, refreshed.RefreshToken, cobotusr.TokenTypeRefreshToken); err != nil {
		t.Fatalf("revoke: %v", err)
	}
	if _, err := client.Refresh(ctx, refreshed.RefreshToken); err == nil {
		t.Fatal("refresh succeeded after the refresh token was revoked")
	} else {
		var sdkErr *cobotusr.Error
		if errors.As(err, &sdkErr) {
			t.Logf("refresh after revocation rejected with %q", sdkErr.Code)
		}
	}

	endSession, err := client.EndSessionURL(ctx, cobotusr.EndSessionOptions{
		PostLogoutRedirectURI: cfg.postLogoutRedirectURI,
		IDTokenHint:           tokens.IDToken,
	})
	if err != nil {
		t.Fatalf("end session url: %v", err)
	}
	endSessionURL, err := url.Parse(endSession)
	if err != nil {
		t.Fatalf("parse end session url: %v", err)
	}
	if got := endSessionURL.Query().Get("client_id"); got != clientID {
		t.Errorf("end session client_id = %q, want %q", got, clientID)
	}
	if got := endSessionURL.Query().Get("post_logout_redirect_uri"); got != cfg.postLogoutRedirectURI {
		t.Errorf("end session post_logout_redirect_uri = %q, want %q", got, cfg.postLogoutRedirectURI)
	}
}

type liveSession struct {
	UserID        int    `json:"user_id"`
	Username      string `json:"username"`
	Administrator bool   `json:"administrator"`
	CSRFToken     string `json:"csrf_token"`
}

type liveBrowser struct {
	t      *testing.T
	base   *url.URL
	origin string
	client *http.Client
	csrf   string
}

func newLiveBrowser(t *testing.T, issuer string) *liveBrowser {
	t.Helper()
	base, err := url.Parse(issuer)
	if err != nil {
		t.Fatalf("parse issuer: %v", err)
	}
	jar, err := cookiejar.New(nil)
	if err != nil {
		t.Fatalf("cookie jar: %v", err)
	}
	return &liveBrowser{
		t:      t,
		base:   base,
		origin: base.Scheme + "://" + base.Host,
		client: &http.Client{
			Jar:           jar,
			Timeout:       30 * time.Second,
			CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
		},
	}
}

func (b *liveBrowser) do(ctx context.Context, method, target string, body []byte, contentType string) (*http.Response, []byte) {
	b.t.Helper()
	var reader io.Reader
	if body != nil {
		reader = bytes.NewReader(body)
	}
	request, err := http.NewRequestWithContext(ctx, method, target, reader)
	if err != nil {
		b.t.Fatalf("%s %s: %v", method, target, err)
	}
	request.Header.Set("Origin", b.origin)
	if contentType != "" {
		request.Header.Set("Content-Type", contentType)
	}
	if b.csrf != "" {
		request.Header.Set("x-csrf-token", b.csrf)
	}
	response, err := b.client.Do(request)
	if err != nil {
		b.t.Fatalf("%s %s: %v", method, target, err)
	}
	defer response.Body.Close()
	data, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		b.t.Fatalf("%s %s: read body: %v", method, target, err)
	}
	return response, data
}

func (b *liveBrowser) fetchCSRF(ctx context.Context) {
	b.t.Helper()
	response, body := b.do(ctx, http.MethodGet, b.base.String()+"/api/v1/identity/csrf", nil, "")
	if response.StatusCode != http.StatusOK {
		b.t.Fatalf("csrf endpoint returned %d: %s", response.StatusCode, body)
	}
	var payload struct {
		CSRFToken string `json:"csrf_token"`
	}
	if err := json.Unmarshal(body, &payload); err != nil {
		b.t.Fatalf("decode csrf response: %v", err)
	}
	if payload.CSRFToken == "" {
		b.t.Fatal("csrf endpoint returned an empty token")
	}
	b.csrf = payload.CSRFToken
}

func (b *liveBrowser) login(ctx context.Context, username, password string) liveSession {
	b.t.Helper()
	b.fetchCSRF(ctx)
	payload, err := json.Marshal(map[string]string{"username": username, "password": password})
	if err != nil {
		b.t.Fatalf("encode login request: %v", err)
	}
	response, body := b.do(ctx, http.MethodPost, b.base.String()+"/api/v1/identity/login", payload, "application/json")
	if response.StatusCode != http.StatusOK {
		b.t.Fatalf("login returned %d: %s", response.StatusCode, body)
	}
	var session liveSession
	if err := json.Unmarshal(body, &session); err != nil {
		b.t.Fatalf("decode login response: %v", err)
	}
	if session.CSRFToken == "" {
		b.t.Fatal("login response is missing csrf_token")
	}
	if session.UserID == 0 {
		b.t.Fatal("login response is missing user_id")
	}
	b.csrf = session.CSRFToken
	return session
}

// registerPublicApp creates a real, governed application through the admin API
// and returns the development client_id. The client is a browser SPA kind, so
// the code exchange is PKCE-only and never needs a client secret.
func (b *liveBrowser) registerPublicApp(ctx context.Context, ownerUserID int, cfg liveConfig) string {
	b.t.Helper()
	redirect, err := url.Parse(cfg.redirectURI)
	if err != nil {
		b.t.Fatalf("parse redirect uri: %v", err)
	}
	origin := redirect.Scheme + "://" + redirect.Host
	appKey := fmt.Sprintf("sdk-it-%d", time.Now().UnixNano())

	environment := func(redirectURI, postLogoutURI, browserOrigin string) map[string]any {
		return map[string]any{
			"client_kind":               "browser_spa",
			"redirect_uris":             []string{redirectURI},
			"post_logout_redirect_uris": []string{postLogoutURI},
			"browser_origins":           []string{browserOrigin},
			"access_audience":           liveAudience,
			"identity_scopes":           strings.Join(cfg.scopes, " "),
			"consent_policy":            "user_consent",
			"logout_capability":         "online",
			"backchannel_logout_uri":    nil,
		}
	}
	payload, err := json.Marshal(map[string]any{
		"application": map[string]any{
			"app_key":     appKey,
			"name":        "Go SDK live integration " + appKey,
			"description": "Created by examples/sdk/go integration test",
			"third_party": false,
			"development": environment(cfg.redirectURI, cfg.postLogoutRedirectURI, origin),
			"production":  environment("https://sdk-it.example/callback", "https://sdk-it.example/", "https://sdk-it.example"),
		},
		"owner_user_id":            ownerUserID,
		"assignments":              []any{},
		"accept_logout_limitation": true,
	})
	if err != nil {
		b.t.Fatalf("encode application registration: %v", err)
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, b.base.String()+"/api/v1/admin/apps", bytes.NewReader(payload))
	if err != nil {
		b.t.Fatalf("application registration request: %v", err)
	}
	request.Header.Set("Origin", b.origin)
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("x-csrf-token", b.csrf)
	request.Header.Set("idempotency-key", appKey)
	response, err := b.client.Do(request)
	if err != nil {
		b.t.Fatalf("register application: %v", err)
	}
	defer response.Body.Close()
	body, err := io.ReadAll(io.LimitReader(response.Body, 1<<20))
	if err != nil {
		b.t.Fatalf("read application registration response: %v", err)
	}
	if response.StatusCode != http.StatusOK {
		b.t.Fatalf("application registration returned %d: %s", response.StatusCode, body)
	}
	var registration struct {
		Environments []struct {
			Kind     string `json:"kind"`
			ClientID string `json:"client_id"`
		} `json:"environments"`
	}
	if err := json.Unmarshal(body, &registration); err != nil {
		b.t.Fatalf("decode application registration: %v", err)
	}
	for _, environment := range registration.Environments {
		if environment.Kind == "development" && environment.ClientID != "" {
			return environment.ClientID
		}
	}
	b.t.Fatalf("application registration returned no development client_id: %s", body)
	return ""
}

func (b *liveBrowser) authorize(ctx context.Context, authorizeURL string) *url.URL {
	b.t.Helper()
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, authorizeURL, nil)
	if err != nil {
		b.t.Fatalf("authorize request: %v", err)
	}
	request.Header.Set("Origin", b.origin)
	response, err := b.client.Do(request)
	if err != nil {
		b.t.Fatalf("authorize: %v", err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusFound && response.StatusCode != http.StatusSeeOther {
		body, _ := io.ReadAll(io.LimitReader(response.Body, 1<<20))
		b.t.Fatalf("authorize returned %d: %s", response.StatusCode, body)
	}
	location, err := url.Parse(response.Header.Get("Location"))
	if err != nil {
		b.t.Fatalf("parse authorize redirect: %v", err)
	}
	return location
}

func (b *liveBrowser) approveConsent(ctx context.Context, flowID string) *url.URL {
	b.t.Helper()
	response, body := b.do(ctx, http.MethodGet, b.base.String()+"/api/v1/identity/flows/"+flowID, nil, "")
	if response.StatusCode != http.StatusOK {
		b.t.Fatalf("flow inspect returned %d: %s", response.StatusCode, body)
	}
	var summary struct {
		CSRFToken string `json:"csrf_token"`
	}
	if err := json.Unmarshal(body, &summary); err != nil {
		b.t.Fatalf("decode flow summary: %v", err)
	}
	if summary.CSRFToken == "" {
		b.t.Fatal("flow summary is missing csrf_token")
	}
	b.csrf = summary.CSRFToken

	payload, err := json.Marshal(map[string]bool{"approved": true})
	if err != nil {
		b.t.Fatalf("encode consent request: %v", err)
	}
	response, body = b.do(ctx, http.MethodPost, b.base.String()+"/api/v1/identity/flows/"+flowID+"/complete", payload, "application/json")
	if response.StatusCode != http.StatusOK {
		b.t.Fatalf("flow complete returned %d: %s", response.StatusCode, body)
	}
	var completed struct {
		RedirectURL string `json:"redirect_url"`
	}
	if err := json.Unmarshal(body, &completed); err != nil {
		b.t.Fatalf("decode flow completion: %v", err)
	}
	location, err := url.Parse(completed.RedirectURL)
	if err != nil {
		b.t.Fatalf("parse flow redirect: %v", err)
	}
	return location
}

func liveConsentFlowID(location *url.URL) string {
	if location.Path != "/oidc/consent" {
		return ""
	}
	return location.Query().Get("flow_id")
}

func liveContains(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
