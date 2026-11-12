package yufu

import (
	"context"
	"crypto"
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/Bin-hy/yufu-sdk-go/internal/pkce"
)

type testSigner struct {
	key *rsa.PrivateKey
	kid string
}

func newTestSigner(t *testing.T, kid string) *testSigner {
	t.Helper()
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatalf("generate RSA key: %v", err)
	}
	return &testSigner{key: key, kid: kid}
}

func (s *testSigner) sign(t *testing.T, typ string, claims map[string]any) string {
	t.Helper()
	headerJSON, err := json.Marshal(map[string]any{"alg": "RS256", "kid": s.kid, "typ": typ})
	if err != nil {
		t.Fatalf("marshal header: %v", err)
	}
	claimsJSON, err := json.Marshal(claims)
	if err != nil {
		t.Fatalf("marshal claims: %v", err)
	}
	signingInput := base64.RawURLEncoding.EncodeToString(headerJSON) + "." +
		base64.RawURLEncoding.EncodeToString(claimsJSON)
	digest := sha256.Sum256([]byte(signingInput))
	signature, err := rsa.SignPKCS1v15(rand.Reader, s.key, crypto.SHA256, digest[:])
	if err != nil {
		t.Fatalf("sign token: %v", err)
	}
	return signingInput + "." + base64.RawURLEncoding.EncodeToString(signature)
}

func (s *testSigner) jwks() []byte {
	modulus := base64.RawURLEncoding.EncodeToString(s.key.PublicKey.N.Bytes())
	exponent := base64.RawURLEncoding.EncodeToString(big.NewInt(int64(s.key.PublicKey.E)).Bytes())
	data, _ := json.Marshal(map[string]any{
		"keys": []map[string]any{{
			"kty": "RSA",
			"use": "sig",
			"alg": "RS256",
			"kid": s.kid,
			"n":   modulus,
			"e":   exponent,
		}},
	})
	return data
}

func writeJSON(t *testing.T, w http.ResponseWriter, value any) {
	t.Helper()
	w.Header().Set("Content-Type", "application/json")
	if err := json.NewEncoder(w).Encode(value); err != nil {
		t.Errorf("encode response: %v", err)
	}
}

func discoveryDocument(issuer string) map[string]any {
	return map[string]any{
		"issuer":                   issuer,
		"authorization_endpoint":   issuer + "/oidc/authorize",
		"token_endpoint":           issuer + "/oidc/token",
		"userinfo_endpoint":        issuer + "/oidc/userinfo",
		"jwks_uri":                 issuer + "/.well-known/jwks.json",
		"revocation_endpoint":      issuer + "/oidc/revoke",
		"introspection_endpoint":   issuer + "/oidc/introspect",
		"end_session_endpoint":     issuer + "/oidc/end-session",
		"response_types_supported": []string{"code"},
		"grant_types_supported":    []string{"authorization_code", "refresh_token"},
		"token_endpoint_auth_methods_supported": []string{
			"client_secret_post", "client_secret_basic", "none",
		},
		"scopes_supported":                     []string{"openid", "profile", "email"},
		"code_challenge_methods_supported":     []string{"S256"},
		"backchannel_logout_supported":         true,
		"backchannel_logout_session_supported": true,
	}
}

func newTestClient(t *testing.T, issuer, clientID, secret string) *Client {
	t.Helper()
	client, err := New(Config{
		Issuer:       issuer,
		ClientID:     clientID,
		ClientSecret: secret,
		RedirectURI:  "https://app.example/callback",
		Scopes:       []string{"openid", "profile", "email"},
	})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	t.Cleanup(func() { _ = client.Close() })
	return client
}

func TestAuthorizeURLUsesDiscoveryAndPKCE(t *testing.T) {
	var issuer string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	}))
	t.Cleanup(server.Close)
	issuer = server.URL

	client := newTestClient(t, issuer, "web", "secret")
	request, err := client.AuthorizeURL(AuthorizeOptions{Scopes: []string{"openid", "profile"}})
	if err != nil {
		t.Fatalf("AuthorizeURL: %v", err)
	}
	parsed, err := url.Parse(request.URL)
	if err != nil {
		t.Fatalf("parse authorization URL: %v", err)
	}
	if got, want := parsed.Scheme+"://"+parsed.Host+parsed.Path, issuer+"/oidc/authorize"; got != want {
		t.Fatalf("authorization endpoint = %q, want %q", got, want)
	}
	query := parsed.Query()
	for key, want := range map[string]string{
		"client_id":             "web",
		"redirect_uri":          "https://app.example/callback",
		"response_type":         "code",
		"code_challenge_method": "S256",
		"scope":                 "openid profile",
		"state":                 request.State,
		"nonce":                 request.Nonce,
	} {
		if got := query.Get(key); got != want {
			t.Fatalf("%s = %q, want %q", key, got, want)
		}
	}
	if request.State == "" || request.Nonce == "" {
		t.Fatal("state and nonce must be generated")
	}
	if err := pkce.ValidateVerifier(request.CodeVerifier); err != nil {
		t.Fatalf("generated verifier is invalid: %v", err)
	}
	if got, want := query.Get("code_challenge"), pkce.Challenge(request.CodeVerifier); got != want {
		t.Fatalf("code_challenge = %q, want %q", got, want)
	}
}

func TestExchangeVerifyUserInfoAndRefresh(t *testing.T) {
	signer := newTestSigner(t, "key-1")
	var issuer string
	var authorizationRequest *AuthorizationRequest
	var mu sync.Mutex
	refreshCount := 0

	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	})
	mux.HandleFunc("/.well-known/jwks.json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(signer.jwks())
	})
	mux.HandleFunc("/oidc/token", func(w http.ResponseWriter, r *http.Request) {
		if err := r.ParseForm(); err != nil {
			http.Error(w, "bad form", http.StatusBadRequest)
			return
		}
		now := time.Now().Unix()
		switch r.Form.Get("grant_type") {
		case "authorization_code":
			mu.Lock()
			auth := authorizationRequest
			mu.Unlock()
			if got := r.Form.Get("client_id"); got != "web" {
				t.Errorf("client_id = %q", got)
			}
			if got := r.Form.Get("client_secret"); got != "secret" {
				t.Errorf("client_secret = %q", got)
			}
			if got := r.Form.Get("code"); got != "code-1" {
				t.Errorf("code = %q", got)
			}
			if auth == nil || r.Form.Get("code_verifier") != auth.CodeVerifier {
				t.Errorf("code_verifier does not match the authorization request")
			}
			accessToken := signer.sign(t, "at+jwt", map[string]any{
				"iss":         issuer,
				"sub":         "42",
				"aud":         "urn:app:test",
				"iat":         now,
				"exp":         now + 300,
				"jti":         "jti-1",
				"scope":       "openid profile",
				"token_use":   "user_session",
				"application": "urn:app:test",
				"session_id":  "session-1",
			})
			idToken := signer.sign(t, "JWT", map[string]any{
				"iss":   issuer,
				"sub":   "42",
				"aud":   "web",
				"iat":   now,
				"exp":   now + 300,
				"nonce": auth.Nonce,
				"sid":   "sid-1",
			})
			writeJSON(t, w, map[string]any{
				"access_token":  accessToken,
				"id_token":      idToken,
				"refresh_token": "refresh-1",
				"token_type":    "Bearer",
				"expires_in":    300,
				"scope":         "openid profile",
			})
		case "refresh_token":
			mu.Lock()
			refreshCount++
			mu.Unlock()
			accessToken := signer.sign(t, "at+jwt", map[string]any{
				"iss": issuer,
				"sub": "42",
				"aud": "urn:app:test",
				"iat": now,
				"exp": now + 300,
				"jti": "jti-2",
			})
			writeJSON(t, w, map[string]any{
				"access_token":  accessToken,
				"refresh_token": "refresh-2",
				"token_type":    "Bearer",
				"expires_in":    300,
			})
		default:
			http.Error(w, "unsupported grant", http.StatusBadRequest)
		}
	})
	mux.HandleFunc("/oidc/userinfo", func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasPrefix(r.Header.Get("Authorization"), "Bearer ") {
			http.Error(w, "missing bearer", http.StatusUnauthorized)
			return
		}
		writeJSON(t, w, map[string]any{
			"sub":                "42",
			"preferred_username": "alice",
			"name":               "Alice",
			"email":              "alice@example.com",
		})
	})
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)
	issuer = server.URL

	client := newTestClient(t, issuer, "web", "secret")
	auth, err := client.AuthorizeURL(AuthorizeOptions{})
	if err != nil {
		t.Fatalf("AuthorizeURL: %v", err)
	}
	mu.Lock()
	authorizationRequest = auth
	mu.Unlock()

	tokens, err := client.ExchangeCode(context.Background(), "code-1", auth.CodeVerifier)
	if err != nil {
		t.Fatalf("ExchangeCode: %v", err)
	}
	if tokens.AccessToken == "" || tokens.IDToken == "" || tokens.RefreshToken != "refresh-1" {
		t.Fatalf("unexpected token set: %+v", tokens)
	}
	if tokens.ExpiresAt.IsZero() {
		t.Fatal("ExpiresAt must be populated")
	}

	idClaims, err := client.VerifyIDToken(context.Background(), tokens.IDToken, auth.Nonce)
	if err != nil {
		t.Fatalf("VerifyIDToken: %v", err)
	}
	if idClaims.Subject != "42" || idClaims.SID != "sid-1" || idClaims.Audience != "web" {
		t.Fatalf("unexpected ID token claims: %+v", idClaims)
	}
	if _, err := client.VerifyIDToken(context.Background(), tokens.IDToken, "wrong-nonce"); err == nil {
		t.Fatal("VerifyIDToken accepted a wrong nonce")
	}

	accessClaims, err := client.VerifyAccessToken(context.Background(), tokens.AccessToken, "urn:app:test")
	if err != nil {
		t.Fatalf("VerifyAccessToken: %v", err)
	}
	if accessClaims.Subject != "42" || accessClaims.TokenUse != "user_session" ||
		accessClaims.SessionID != "session-1" || accessClaims.Scope != "openid profile" {
		t.Fatalf("unexpected access token claims: %+v", accessClaims)
	}
	if _, err := client.VerifyAccessToken(context.Background(), tokens.AccessToken, "urn:other"); err == nil {
		t.Fatal("VerifyAccessToken accepted a wrong audience")
	}
	if _, err := client.VerifyAccessToken(context.Background(), tokens.IDToken, "urn:app:test"); err == nil {
		t.Fatal("VerifyAccessToken accepted an ID token")
	}

	info, err := client.UserInfo(context.Background(), tokens.AccessToken)
	if err != nil {
		t.Fatalf("UserInfo: %v", err)
	}
	if info.Subject != "42" || info.PreferredUsername != "alice" || info.Email != "alice@example.com" {
		t.Fatalf("unexpected userinfo: %+v", info)
	}

	refreshed, err := client.Refresh(context.Background(), tokens.RefreshToken)
	if err != nil {
		t.Fatalf("Refresh: %v", err)
	}
	if refreshed.RefreshToken != "refresh-2" || refreshed.AccessToken == "" {
		t.Fatalf("unexpected refreshed token set: %+v", refreshed)
	}
	mu.Lock()
	defer mu.Unlock()
	if refreshCount != 1 {
		t.Fatalf("refresh requests = %d, want 1", refreshCount)
	}
}

func TestRefreshIsSingleFlight(t *testing.T) {
	signer := newTestSigner(t, "key-1")
	var issuer string
	var refreshCount int32

	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	})
	mux.HandleFunc("/oidc/token", func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&refreshCount, 1)
		time.Sleep(50 * time.Millisecond)
		now := time.Now().Unix()
		writeJSON(t, w, map[string]any{
			"access_token": signer.sign(t, "at+jwt", map[string]any{
				"iss": issuer,
				"sub": "42",
				"aud": "urn:app:test",
				"iat": now,
				"exp": now + 300,
			}),
			"refresh_token": "refresh-2",
			"token_type":    "Bearer",
			"expires_in":    300,
		})
	})
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)
	issuer = server.URL

	client := newTestClient(t, issuer, "web", "secret")
	var wait sync.WaitGroup
	errs := make(chan error, 5)
	for i := 0; i < 5; i++ {
		wait.Add(1)
		go func() {
			defer wait.Done()
			_, err := client.Refresh(context.Background(), "refresh-1")
			errs <- err
		}()
	}
	wait.Wait()
	close(errs)
	for err := range errs {
		if err != nil {
			t.Fatalf("Refresh: %v", err)
		}
	}
	if got := atomic.LoadInt32(&refreshCount); got != 1 {
		t.Fatalf("refresh requests = %d, want 1", got)
	}
}

func TestIntrospectAndRevoke(t *testing.T) {
	var issuer string
	var mu sync.Mutex
	var revoked string

	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	})
	mux.HandleFunc("/oidc/introspect", func(w http.ResponseWriter, r *http.Request) {
		if err := r.ParseForm(); err != nil {
			http.Error(w, "bad form", http.StatusBadRequest)
			return
		}
		if got := r.Form.Get("client_id"); got != "reader" {
			t.Errorf("introspection client_id = %q", got)
		}
		if got := r.Form.Get("client_secret"); got != "reader-secret" {
			t.Errorf("introspection client_secret = %q", got)
		}
		if got := r.Form.Get("token"); got != "access-1" {
			t.Errorf("introspection token = %q", got)
		}
		writeJSON(t, w, map[string]any{
			"active":     true,
			"scope":      "openid profile",
			"client_id":  "web",
			"username":   "alice",
			"token_type": "Bearer",
			"exp":        time.Now().Add(5 * time.Minute).Unix(),
			"sub":        "42",
			"sid":        "sid-1",
			"aud":        "urn:app:test",
			"iss":        issuer,
		})
	})
	mux.HandleFunc("/oidc/revoke", func(w http.ResponseWriter, r *http.Request) {
		if err := r.ParseForm(); err != nil {
			http.Error(w, "bad form", http.StatusBadRequest)
			return
		}
		mu.Lock()
		revoked = r.Form.Get("token")
		mu.Unlock()
		w.WriteHeader(http.StatusOK)
	})
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)
	issuer = server.URL

	reader := newTestClient(t, issuer, "reader", "reader-secret")
	info, err := reader.Introspect(context.Background(), "access-1", TokenTypeAccessToken)
	if err != nil {
		t.Fatalf("Introspect: %v", err)
	}
	if !info.Active || info.Subject != "42" || info.ClientID != "web" ||
		info.SID != "sid-1" || info.Audience != "urn:app:test" {
		t.Fatalf("unexpected introspection: %+v", info)
	}
	if err := reader.Revoke(context.Background(), "access-1", TokenTypeAccessToken); err != nil {
		t.Fatalf("Revoke: %v", err)
	}
	mu.Lock()
	defer mu.Unlock()
	if revoked != "access-1" {
		t.Fatalf("revoked token = %q", revoked)
	}
}

func TestVerifyLogoutToken(t *testing.T) {
	signer := newTestSigner(t, "key-1")
	var issuer string
	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	})
	mux.HandleFunc("/.well-known/jwks.json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(signer.jwks())
	})
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)
	issuer = server.URL

	client := newTestClient(t, issuer, "web", "secret")
	now := time.Now().Unix()
	validClaims := func() map[string]any {
		return map[string]any{
			"iss": issuer,
			"aud": "web",
			"iat": now,
			"exp": now + 120,
			"jti": "jti-1",
			"sub": "42",
			"sid": "sid-1",
			"events": map[string]any{
				backchannelLogoutEvent: map[string]any{},
			},
		}
	}
	token := signer.sign(t, "logout+jwt", validClaims())
	claims, err := client.VerifyLogoutToken(context.Background(), token)
	if err != nil {
		t.Fatalf("VerifyLogoutToken: %v", err)
	}
	if claims.Subject != "42" || claims.SID != "sid-1" || claims.JTI != "jti-1" {
		t.Fatalf("unexpected logout claims: %+v", claims)
	}

	invalid := map[string]func(map[string]any){
		"nonce":    func(claims map[string]any) { claims["nonce"] = "x" },
		"lifetime": func(claims map[string]any) { claims["exp"] = now + 121 },
		"audience": func(claims map[string]any) { claims["aud"] = "other" },
		"events":   func(claims map[string]any) { delete(claims, "events") },
		"event-payload": func(claims map[string]any) {
			claims["events"] = map[string]any{backchannelLogoutEvent: map[string]any{"x": true}}
		},
		"subject": func(claims map[string]any) {
			delete(claims, "sub")
			delete(claims, "sid")
		},
	}
	for name, mutate := range invalid {
		t.Run(name, func(t *testing.T) {
			claims := validClaims()
			mutate(claims)
			if _, err := client.VerifyLogoutToken(context.Background(), signer.sign(t, "logout+jwt", claims)); err == nil {
				t.Fatal("VerifyLogoutToken accepted an invalid token")
			}
		})
	}
	if _, err := client.VerifyLogoutToken(context.Background(), signer.sign(t, "JWT", validClaims())); err == nil {
		t.Fatal("VerifyLogoutToken accepted a non-logout typ")
	}
}

func TestErrorMapping(t *testing.T) {
	var issuer string
	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	})
	mux.HandleFunc("/oidc/token", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusBadRequest)
		_, _ = w.Write([]byte(`{"error":"invalid_grant","request_id":"req-123"}`))
	})
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)
	issuer = server.URL

	client := newTestClient(t, issuer, "web", "secret")
	verifier, err := pkce.NewVerifier()
	if err != nil {
		t.Fatal(err)
	}
	_, err = client.ExchangeCode(context.Background(), "code-1", verifier)
	if err == nil {
		t.Fatal("ExchangeCode succeeded unexpectedly")
	}
	var sdkError *Error
	if !errors.As(err, &sdkError) {
		t.Fatalf("error type = %T, want *Error", err)
	}
	if sdkError.StatusCode != http.StatusBadRequest || sdkError.Code != "invalid_grant" ||
		sdkError.RequestID != "req-123" {
		t.Fatalf("unexpected error: %+v", sdkError)
	}
}

func TestJWKSRotation(t *testing.T) {
	first := newTestSigner(t, "key-1")
	second := newTestSigner(t, "key-2")
	var issuer string
	var mu sync.Mutex
	current := first

	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	})
	mux.HandleFunc("/.well-known/jwks.json", func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		signer := current
		mu.Unlock()
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(signer.jwks())
	})
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)
	issuer = server.URL

	client := newTestClient(t, issuer, "web", "secret")
	now := time.Now().Unix()
	claims := func() map[string]any {
		return map[string]any{
			"iss": issuer,
			"sub": "42",
			"aud": "urn:app:test",
			"iat": now,
			"exp": now + 300,
		}
	}
	if _, err := client.VerifyAccessToken(context.Background(), first.sign(t, "at+jwt", claims()), "urn:app:test"); err != nil {
		t.Fatalf("verify first key: %v", err)
	}
	mu.Lock()
	current = second
	mu.Unlock()
	if _, err := client.VerifyAccessToken(context.Background(), second.sign(t, "at+jwt", claims()), "urn:app:test"); err != nil {
		t.Fatalf("verify rotated key: %v", err)
	}
}

func TestDiscoveryIssuerMismatch(t *testing.T) {
	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument("https://other.example"))
	})
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)

	client := newTestClient(t, server.URL, "web", "secret")
	_, err := client.Discover(context.Background())
	if err == nil {
		t.Fatal("Discover accepted a mismatched issuer")
	}
	if !IsErrorCode(err, "invalid_discovery") {
		t.Fatalf("error = %v, want invalid_discovery", err)
	}
}

func TestEndSessionURL(t *testing.T) {
	var issuer string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		writeJSON(t, w, discoveryDocument(issuer))
	}))
	t.Cleanup(server.Close)
	issuer = server.URL
	client := newTestClient(t, issuer, "web", "secret")
	endURL, err := client.EndSessionURL(context.Background(), EndSessionOptions{
		PostLogoutRedirectURI: "https://app.example/",
		IDTokenHint:           "id-token",
		State:                 "state-1",
	})
	if err != nil {
		t.Fatalf("EndSessionURL: %v", err)
	}
	parsed, err := url.Parse(endURL)
	if err != nil {
		t.Fatal(err)
	}
	query := parsed.Query()
	if query.Get("client_id") != "web" ||
		query.Get("post_logout_redirect_uri") != "https://app.example/" ||
		query.Get("id_token_hint") != "id-token" ||
		query.Get("state") != "state-1" {
		t.Fatalf("unexpected end-session query: %v", query)
	}
}

func TestConfigValidation(t *testing.T) {
	for name, cfg := range map[string]Config{
		"missing issuer":    {ClientID: "web"},
		"relative issuer":   {Issuer: "/auth", ClientID: "web"},
		"missing client id": {Issuer: "https://auth.example"},
		"basic without secret": {
			Issuer: "https://auth.example", ClientID: "web", TokenAuthMethod: TokenAuthBasic,
		},
		"none with secret": {
			Issuer: "https://auth.example", ClientID: "web", ClientSecret: "secret", TokenAuthMethod: TokenAuthNone,
		},
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := New(cfg); err == nil {
				t.Fatal("New accepted an invalid config")
			}
		})
	}
}

func TestTokenSetRejectsNonBearer(t *testing.T) {
	client, err := New(Config{Issuer: "https://auth.example", ClientID: "web"})
	if err != nil {
		t.Fatal(err)
	}
	_, err = client.tokenSet(tokenResponse{AccessToken: "token", TokenType: "mac"})
	if err == nil {
		t.Fatal("tokenSet accepted a non-Bearer token type")
	}
}

func TestReadLimited(t *testing.T) {
	if _, err := readLimited(strings.NewReader(strings.Repeat("x", 11)), 10); err == nil {
		t.Fatal("readLimited accepted an oversized body")
	}
	if data, err := readLimited(strings.NewReader("ok"), 10); err != nil || string(data) != "ok" {
		t.Fatalf("readLimited = %q, %v", data, err)
	}
}

func TestIntrospectionInactive(t *testing.T) {
	info := introspectionFrom(map[string]any{"active": false})
	if info.Active || info.Subject != "" || info.SID != "" {
		t.Fatalf("inactive introspection leaked metadata: %+v", info)
	}
}

func TestClaimsHelpers(t *testing.T) {
	claims := map[string]any{
		"iss":   "https://auth.example",
		"sub":   "42",
		"aud":   []any{"a", "b"},
		"exp":   json.Number("123"),
		"nbf":   json.Number("100"),
		"iat":   json.Number("90"),
		"jti":   "jti-1",
		"scope": "openid",
		"roles": []any{"admin", "user"},
	}
	access, err := accessTokenClaimsFrom(claims)
	if err != nil {
		t.Fatal(err)
	}
	if access.Audience != "a" || len(access.Audiences) != 2 ||
		access.ExpiresAt.Unix() != 123 || access.NotBefore.Unix() != 100 ||
		access.IssuedAt.Unix() != 90 || len(access.Roles) != 2 {
		t.Fatalf("unexpected claims: %+v", access)
	}
	if _, err := accessTokenClaimsFrom(map[string]any{"sub": "42"}); err == nil {
		t.Fatal("accessTokenClaimsFrom accepted a token without aud")
	}
	if _, err := idTokenClaimsFrom(map[string]any{"sub": "42"}); err == nil {
		t.Fatal("idTokenClaimsFrom accepted a token without aud")
	}
}

func TestErrorTemporary(t *testing.T) {
	for status, want := range map[int]bool{
		400: false, 401: false, 429: true, 500: true, 503: true,
	} {
		if got := (&Error{StatusCode: status}).Temporary(); got != want {
			t.Fatalf("Temporary(%d) = %v, want %v", status, got, want)
		}
	}
	if !(&Error{Err: fmt.Errorf("network")}).Temporary() {
		t.Fatal("network errors must be temporary")
	}
}

func TestNewRejectsMissingConfiguration(t *testing.T) {
	if _, err := New(Config{}); err == nil {
		t.Fatal("New accepted an empty config")
	}
	if _, err := New(Config{Issuer: "https://auth.example"}); err == nil {
		t.Fatal("New accepted a config without ClientID")
	}
	if _, err := New(Config{Issuer: "not-a-url", ClientID: "client"}); err == nil {
		t.Fatal("New accepted a non-absolute issuer")
	}
	if _, err := New(Config{Issuer: "https://auth.example", ClientID: "client", TokenAuthMethod: TokenAuthBasic}); err == nil {
		t.Fatal("New accepted client_secret_basic without a secret")
	}
	if _, err := New(Config{Issuer: "https://auth.example", ClientID: "client", ClientSecret: "secret", TokenAuthMethod: TokenAuthNone}); err == nil {
		t.Fatal("New accepted TokenAuthNone with a secret")
	}
	client, err := New(Config{Issuer: "https://auth.example", ClientID: "client"})
	if err != nil {
		t.Fatalf("New rejected a valid public client config: %v", err)
	}
	defer client.Close()
}
