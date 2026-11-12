// Command sdk-go-example is a runnable Runtime SDK integration example.
//
// It starts a small web application that performs the authorization-code +
// PKCE flow, verifies tokens locally, calls UserInfo, and optionally
// introspects bearer tokens with a Reader client.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"net/http"
	"os"
	"strings"
	"sync"
	"time"

	yufu "github.com/Bin-hy/yufu-sdk-go"
)

type config struct {
	issuer                string
	clientID              string
	clientSecret          string
	redirectURI           string
	postLogoutRedirectURI string
	audience              string
	scopes                []string
	readerClientID        string
	readerClientSecret    string
	addr                  string
}

type demo struct {
	cfg    config
	client *yufu.Client
	reader *yufu.Client

	mu      sync.Mutex
	pending map[string]*yufu.AuthorizationRequest

	logoutMu sync.Mutex
	seenJTI  map[string]time.Time
}

func main() {
	cfg, err := loadConfig()
	if err != nil {
		log.Fatal(err)
	}
	client, err := yufu.New(yufu.Config{
		Issuer:       cfg.issuer,
		ClientID:     cfg.clientID,
		ClientSecret: cfg.clientSecret,
		RedirectURI:  cfg.redirectURI,
		Scopes:       cfg.scopes,
	})
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	var reader *yufu.Client
	if cfg.readerClientID != "" {
		reader, err = yufu.New(yufu.Config{
			Issuer:       cfg.issuer,
			ClientID:     cfg.readerClientID,
			ClientSecret: cfg.readerClientSecret,
		})
		if err != nil {
			log.Fatal(err)
		}
		defer reader.Close()
	}

	app := &demo{
		cfg:     cfg,
		client:  client,
		reader:  reader,
		pending: make(map[string]*yufu.AuthorizationRequest),
		seenJTI: make(map[string]time.Time),
	}
	mux := http.NewServeMux()
	mux.HandleFunc("/", app.index)
	mux.HandleFunc("/login", app.login)
	mux.HandleFunc("/callback", app.callback)
	mux.HandleFunc("/api/me", app.me)
	mux.HandleFunc("/logout", app.logout)
	mux.HandleFunc("/oidc/backchannel-logout", app.backchannelLogout)
	log.Printf("yufu Go SDK example listening on http://%s", cfg.addr)
	log.Fatal(http.ListenAndServe(cfg.addr, mux))
}

func loadConfig() (config, error) {
	cfg := config{
		issuer:                os.Getenv("ISSUER"),
		clientID:              os.Getenv("CLIENT_ID"),
		clientSecret:          os.Getenv("CLIENT_SECRET"),
		redirectURI:           os.Getenv("REDIRECT_URI"),
		postLogoutRedirectURI: os.Getenv("POST_LOGOUT_REDIRECT_URI"),
		audience:              os.Getenv("AUDIENCE"),
		readerClientID:        os.Getenv("READER_CLIENT_ID"),
		readerClientSecret:    os.Getenv("READER_CLIENT_SECRET"),
		addr:                  os.Getenv("ADDR"),
	}
	if cfg.addr == "" {
		cfg.addr = "127.0.0.1:8080"
	}
	if cfg.postLogoutRedirectURI == "" {
		cfg.postLogoutRedirectURI = cfg.redirectURI
	}
	scopeText := os.Getenv("SCOPES")
	if scopeText == "" {
		scopeText = "openid profile email"
	}
	cfg.scopes = strings.Fields(scopeText)
	for name, value := range map[string]string{
		"ISSUER":       cfg.issuer,
		"CLIENT_ID":    cfg.clientID,
		"REDIRECT_URI": cfg.redirectURI,
		"AUDIENCE":     cfg.audience,
	} {
		if value == "" {
			return config{}, fmt.Errorf("%s is required", name)
		}
	}
	if (cfg.readerClientID == "") != (cfg.readerClientSecret == "") {
		return config{}, errors.New("READER_CLIENT_ID and READER_CLIENT_SECRET must be set together")
	}
	return cfg, nil
}

func (d *demo) index(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	_, _ = fmt.Fprintf(w, `<!doctype html>
<html lang="en"><meta charset="utf-8"><title>yufu Go SDK</title>
<body>
<h1>yufu Go SDK example</h1>
<p><a href="/login">Sign in</a> · <a href="/api/me">Protected API</a> · <a href="/logout">Sign out</a></p>
<p>Issuer: <code>%s</code></p>
</body></html>`, d.cfg.issuer)
}

func (d *demo) login(w http.ResponseWriter, r *http.Request) {
	auth, err := d.client.AuthorizeURL(yufu.AuthorizeOptions{})
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	d.mu.Lock()
	d.pending[auth.State] = auth
	d.mu.Unlock()
	http.Redirect(w, r, auth.URL, http.StatusFound)
}

func (d *demo) callback(w http.ResponseWriter, r *http.Request) {
	state := r.URL.Query().Get("state")
	code := r.URL.Query().Get("code")
	if state == "" || code == "" {
		http.Error(w, "missing state or code", http.StatusBadRequest)
		return
	}
	d.mu.Lock()
	auth := d.pending[state]
	delete(d.pending, state)
	d.mu.Unlock()
	if auth == nil {
		http.Error(w, "unknown or replayed state", http.StatusBadRequest)
		return
	}
	tokens, err := d.client.ExchangeCode(r.Context(), code, auth.CodeVerifier)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	idClaims, err := d.client.VerifyIDToken(r.Context(), tokens.IDToken, auth.Nonce)
	if err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	accessClaims, err := d.client.VerifyAccessToken(r.Context(), tokens.AccessToken, d.cfg.audience)
	if err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	profile, err := d.client.UserInfo(r.Context(), tokens.AccessToken)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"id_token": map[string]any{
			"sub": idClaims.Subject,
			"sid": idClaims.SID,
			"aud": idClaims.Audience,
		},
		"access_token": map[string]any{
			"sub":         accessClaims.Subject,
			"aud":         accessClaims.Audience,
			"token_use":   accessClaims.TokenUse,
			"session_id":  accessClaims.SessionID,
			"expires_at":  accessClaims.ExpiresAt,
			"scope":       accessClaims.Scope,
			"application": accessClaims.Application,
		},
		"userinfo": map[string]any{
			"sub":                profile.Subject,
			"preferred_username": profile.PreferredUsername,
			"name":               profile.Name,
			"email":              profile.Email,
		},
		"expires_in": tokens.ExpiresIn,
	})
}

func (d *demo) me(w http.ResponseWriter, r *http.Request) {
	token, err := bearerToken(r)
	if err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	claims, err := d.client.VerifyAccessToken(r.Context(), token, d.cfg.audience)
	if err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	response := map[string]any{
		"sub":        claims.Subject,
		"aud":        claims.Audience,
		"token_use":  claims.TokenUse,
		"session_id": claims.SessionID,
		"expires_at": claims.ExpiresAt,
	}
	if d.reader != nil {
		introspection, err := d.reader.Introspect(r.Context(), token, yufu.TokenTypeAccessToken)
		if err != nil {
			http.Error(w, err.Error(), http.StatusBadGateway)
			return
		}
		response["introspection"] = map[string]any{
			"active":    introspection.Active,
			"client_id": introspection.ClientID,
			"sub":       introspection.Subject,
			"sid":       introspection.SID,
			"scope":     introspection.Scope,
			"exp":       introspection.ExpiresAt,
		}
	}
	writeJSON(w, http.StatusOK, response)
}

func (d *demo) logout(w http.ResponseWriter, r *http.Request) {
	endURL, err := d.client.EndSessionURL(r.Context(), yufu.EndSessionOptions{
		PostLogoutRedirectURI: d.cfg.postLogoutRedirectURI,
	})
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	http.Redirect(w, r, endURL, http.StatusFound)
}

func (d *demo) backchannelLogout(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "invalid form", http.StatusBadRequest)
		return
	}
	token := r.Form.Get("logout_token")
	if token == "" {
		http.Error(w, "missing logout_token", http.StatusBadRequest)
		return
	}
	claims, err := d.client.VerifyLogoutToken(r.Context(), token)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	d.logoutMu.Lock()
	_, seen := d.seenJTI[claims.JTI]
	if !seen {
		d.seenJTI[claims.JTI] = time.Now()
	}
	d.logoutMu.Unlock()
	if seen {
		// Idempotent acknowledgement for a replayed delivery.
		w.WriteHeader(http.StatusOK)
		return
	}
	// A real application deletes local sessions by claims.SID and claims.Subject.
	w.WriteHeader(http.StatusOK)
}

func bearerToken(r *http.Request) (string, error) {
	header := r.Header.Get("Authorization")
	if header == "" {
		return "", errors.New("missing Authorization header")
	}
	scheme, token, ok := strings.Cut(header, " ")
	if !ok || !strings.EqualFold(scheme, "Bearer") || token == "" {
		return "", errors.New("invalid Authorization header")
	}
	return token, nil
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
