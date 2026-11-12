package main

import (
	"crypto"
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"io"
	"math/big"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"sync"
	"testing"
	"time"

	yufu "github.com/Bin-hy/yufu-sdk-go"
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
		t.Fatal(err)
	}
	claimsJSON, err := json.Marshal(claims)
	if err != nil {
		t.Fatal(err)
	}
	signingInput := base64.RawURLEncoding.EncodeToString(headerJSON) + "." +
		base64.RawURLEncoding.EncodeToString(claimsJSON)
	digest := sha256.Sum256([]byte(signingInput))
	signature, err := rsa.SignPKCS1v15(rand.Reader, s.key, crypto.SHA256, digest[:])
	if err != nil {
		t.Fatal(err)
	}
	return signingInput + "." + base64.RawURLEncoding.EncodeToString(signature)
}

func (s *testSigner) jwks() []byte {
	modulus := base64.RawURLEncoding.EncodeToString(s.key.PublicKey.N.Bytes())
	exponent := base64.RawURLEncoding.EncodeToString(big.NewInt(int64(s.key.PublicKey.E)).Bytes())
	data, _ := json.Marshal(map[string]any{
		"keys": []map[string]any{{
			"kty": "RSA", "use": "sig", "alg": "RS256", "kid": s.kid,
			"n": modulus, "e": exponent,
		}},
	})
	return data
}

type fakeIssuer struct {
	t      *testing.T
	signer *testSigner
	server *httptest.Server

	mu              sync.Mutex
	challenge       string
	nonce           string
	lastAccessToken string
	refreshCount    int
}

func newFakeIssuer(t *testing.T) *fakeIssuer {
	t.Helper()
	f := &fakeIssuer{t: t, signer: newTestSigner(t, "key-1")}
	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, http.StatusOK, map[string]any{
			"issuer":                               f.server.URL,
			"authorization_endpoint":               f.server.URL + "/oidc/authorize",
			"token_endpoint":                       f.server.URL + "/oidc/token",
			"userinfo_endpoint":                    f.server.URL + "/oidc/userinfo",
			"jwks_uri":                             f.server.URL + "/.well-known/jwks.json",
			"revocation_endpoint":                  f.server.URL + "/oidc/revoke",
			"introspection_endpoint":               f.server.URL + "/oidc/introspect",
			"end_session_endpoint":                 f.server.URL + "/oidc/end-session",
			"response_types_supported":             []string{"code"},
			"grant_types_supported":                []string{"authorization_code", "refresh_token"},
			"scopes_supported":                     []string{"openid", "profile", "email"},
			"code_challenge_methods_supported":     []string{"S256"},
			"backchannel_logout_supported":         true,
			"backchannel_logout_session_supported": true,
		})
	})
	mux.HandleFunc("/.well-known/jwks.json", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(f.signer.jwks())
	})
	mux.HandleFunc("/oidc/token", func(w http.ResponseWriter, r *http.Request) {
		if err := r.ParseForm(); err != nil {
			http.Error(w, "bad form", http.StatusBadRequest)
			return
		}
		now := time.Now().Unix()
		switch r.Form.Get("grant_type") {
		case "authorization_code":
			f.mu.Lock()
			challenge, nonce := f.challenge, f.nonce
			f.mu.Unlock()
			verifier := r.Form.Get("code_verifier")
			digest := sha256.Sum256([]byte(verifier))
			if got := base64.RawURLEncoding.EncodeToString(digest[:]); got != challenge {
				t.Errorf("code_challenge mismatch: got %q want %q", got, challenge)
			}
			accessToken := f.signer.sign(t, "at+jwt", map[string]any{
				"iss": f.server.URL, "sub": "42", "aud": "urn:app:test",
				"iat": now, "exp": now + 300, "jti": "jti-1",
				"scope": "openid profile", "token_use": "user_session",
				"application": "urn:app:test", "session_id": "session-1",
			})
			idToken := f.signer.sign(t, "JWT", map[string]any{
				"iss": f.server.URL, "sub": "42", "aud": "web",
				"iat": now, "exp": now + 300, "nonce": nonce, "sid": "sid-1",
			})
			f.mu.Lock()
			f.lastAccessToken = accessToken
			f.mu.Unlock()
			writeJSON(w, http.StatusOK, map[string]any{
				"access_token": accessToken, "id_token": idToken,
				"refresh_token": "refresh-1", "token_type": "Bearer",
				"expires_in": 300, "scope": "openid profile",
			})
		case "refresh_token":
			f.mu.Lock()
			f.refreshCount++
			f.mu.Unlock()
			writeJSON(w, http.StatusOK, map[string]any{
				"access_token": f.signer.sign(t, "at+jwt", map[string]any{
					"iss": f.server.URL, "sub": "42", "aud": "urn:app:test",
					"iat": now, "exp": now + 300, "jti": "jti-2",
				}),
				"refresh_token": "refresh-2", "token_type": "Bearer", "expires_in": 300,
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
		writeJSON(w, http.StatusOK, map[string]any{
			"sub": "42", "preferred_username": "alice", "name": "Alice",
			"email": "alice@example.com",
		})
	})
	mux.HandleFunc("/oidc/introspect", func(w http.ResponseWriter, r *http.Request) {
		if err := r.ParseForm(); err != nil {
			http.Error(w, "bad form", http.StatusBadRequest)
			return
		}
		if r.Form.Get("client_id") != "reader" || r.Form.Get("client_secret") != "reader-secret" {
			http.Error(w, "bad reader credentials", http.StatusUnauthorized)
			return
		}
		writeJSON(w, http.StatusOK, map[string]any{
			"active": true, "scope": "openid profile", "client_id": "web",
			"username": "alice", "token_type": "Bearer",
			"exp": time.Now().Add(5 * time.Minute).Unix(),
			"sub": "42", "sid": "sid-1", "aud": "urn:app:test", "iss": f.server.URL,
		})
	})
	mux.HandleFunc("/oidc/revoke", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	})
	f.server = httptest.NewServer(mux)
	t.Cleanup(f.server.Close)
	return f
}

func (f *fakeIssuer) setAuthorization(challenge, nonce string) {
	f.mu.Lock()
	defer f.mu.Unlock()
	f.challenge = challenge
	f.nonce = nonce
}

func (f *fakeIssuer) accessToken() string {
	f.mu.Lock()
	defer f.mu.Unlock()
	return f.lastAccessToken
}

func (f *fakeIssuer) logoutToken(jti, sid, sub string) string {
	now := time.Now().Unix()
	return f.signer.sign(f.t, "logout+jwt", map[string]any{
		"iss": f.server.URL, "aud": "web", "iat": now, "exp": now + 120,
		"jti": jti, "sid": sid, "sub": sub,
		"events": map[string]any{
			"http://schemas.openid.net/event/backchannel-logout": map[string]any{},
		},
	})
}

func TestExampleAuthorizationCodeFlow(t *testing.T) {
	issuer := newFakeIssuer(t)
	client, err := yufu.New(yufu.Config{
		Issuer: issuer.server.URL, ClientID: "web", ClientSecret: "secret",
		RedirectURI: "https://app.example/callback",
		Scopes:      []string{"openid", "profile", "email"},
	})
	if err != nil {
		t.Fatal(err)
	}
	defer client.Close()
	reader, err := yufu.New(yufu.Config{
		Issuer: issuer.server.URL, ClientID: "reader", ClientSecret: "reader-secret",
	})
	if err != nil {
		t.Fatal(err)
	}
	defer reader.Close()

	app := &demo{
		cfg: config{
			issuer: issuer.server.URL, clientID: "web", clientSecret: "secret",
			redirectURI:           "https://app.example/callback",
			postLogoutRedirectURI: "https://app.example/",
			audience:              "urn:app:test",
			scopes:                []string{"openid", "profile", "email"},
		},
		client:  client,
		reader:  reader,
		pending: make(map[string]*yufu.AuthorizationRequest),
		seenJTI: make(map[string]time.Time),
	}
	mux := http.NewServeMux()
	mux.HandleFunc("/login", app.login)
	mux.HandleFunc("/callback", app.callback)
	mux.HandleFunc("/api/me", app.me)
	mux.HandleFunc("/logout", app.logout)
	mux.HandleFunc("/oidc/backchannel-logout", app.backchannelLogout)
	server := httptest.NewServer(mux)
	t.Cleanup(server.Close)

	noRedirect := &http.Client{CheckRedirect: func(req *http.Request, via []*http.Request) error {
		return http.ErrUseLastResponse
	}}
	response, err := noRedirect.Get(server.URL + "/login")
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusFound {
		t.Fatalf("login status = %d", response.StatusCode)
	}
	authorizeURL, err := url.Parse(response.Header.Get("Location"))
	if err != nil {
		t.Fatal(err)
	}
	state := authorizeURL.Query().Get("state")
	nonce := authorizeURL.Query().Get("nonce")
	challenge := authorizeURL.Query().Get("code_challenge")
	if state == "" || nonce == "" || challenge == "" {
		t.Fatal("authorization URL is missing state, nonce or code_challenge")
	}
	issuer.setAuthorization(challenge, nonce)

	response, err = noRedirect.Get(server.URL + "/callback?code=code-1&state=" + url.QueryEscape(state))
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	body, _ := io.ReadAll(response.Body)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("callback status = %d: %s", response.StatusCode, body)
	}
	var callback map[string]any
	if err := json.Unmarshal(body, &callback); err != nil {
		t.Fatal(err)
	}
	idToken, _ := callback["id_token"].(map[string]any)
	accessToken, _ := callback["access_token"].(map[string]any)
	userinfo, _ := callback["userinfo"].(map[string]any)
	if idToken["sub"] != "42" || idToken["sid"] != "sid-1" {
		t.Fatalf("unexpected ID token result: %v", idToken)
	}
	if accessToken["aud"] != "urn:app:test" || accessToken["token_use"] != "user_session" {
		t.Fatalf("unexpected access token result: %v", accessToken)
	}
	if userinfo["preferred_username"] != "alice" {
		t.Fatalf("unexpected userinfo result: %v", userinfo)
	}

	request, _ := http.NewRequest(http.MethodGet, server.URL+"/api/me", nil)
	request.Header.Set("Authorization", "Bearer "+issuer.accessToken())
	response, err = http.DefaultClient.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	body, _ = io.ReadAll(response.Body)
	if response.StatusCode != http.StatusOK {
		t.Fatalf("api/me status = %d: %s", response.StatusCode, body)
	}
	var me map[string]any
	if err := json.Unmarshal(body, &me); err != nil {
		t.Fatal(err)
	}
	introspection, _ := me["introspection"].(map[string]any)
	if introspection["active"] != true || introspection["client_id"] != "web" {
		t.Fatalf("unexpected introspection: %v", introspection)
	}

	response, err = noRedirect.Get(server.URL + "/logout")
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusFound {
		t.Fatalf("logout status = %d", response.StatusCode)
	}
	endSession, err := url.Parse(response.Header.Get("Location"))
	if err != nil {
		t.Fatal(err)
	}
	if endSession.Query().Get("client_id") != "web" ||
		endSession.Query().Get("post_logout_redirect_uri") != "https://app.example/" {
		t.Fatalf("unexpected end-session URL: %s", endSession)
	}

	logoutToken := issuer.logoutToken("jti-1", "sid-1", "42")
	response, err = http.PostForm(server.URL+"/oidc/backchannel-logout", url.Values{"logout_token": {logoutToken}})
	if err != nil {
		t.Fatal(err)
	}
	response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("backchannel logout status = %d", response.StatusCode)
	}
	// Replay must be acknowledged idempotently.
	response, err = http.PostForm(server.URL+"/oidc/backchannel-logout", url.Values{"logout_token": {logoutToken}})
	if err != nil {
		t.Fatal(err)
	}
	response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("replayed backchannel logout status = %d", response.StatusCode)
	}
	response, err = http.PostForm(server.URL+"/oidc/backchannel-logout", url.Values{"logout_token": {"not-a-token"}})
	if err != nil {
		t.Fatal(err)
	}
	response.Body.Close()
	if response.StatusCode != http.StatusBadRequest {
		t.Fatalf("invalid backchannel logout status = %d", response.StatusCode)
	}
}
