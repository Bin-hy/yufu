# Go Web Backend Integration (Authorization Code + PKCE)

Use this for Go web services that need users to sign in through yufu and keep the session on the server.
Register with `--mode web` (`client_kind = server_web`, with a client secret).

## 1. Get the configuration

| Environment variable | Source |
| --- | --- |
| `ISSUER` | `YUFU_ISSUER` in `state.env` |
| `CLIENT_ID` | `YUFU_CLIENT_ID` in `state.env` |
| `CLIENT_SECRET` | Contents of the file named by `YUFU_CLIENT_SECRET_FILE` |
| `REDIRECT_URI` | `YUFU_REDIRECT_URI` from registration; must match byte for byte |
| `AUDIENCE` | `YUFU_AUDIENCE` in `state.env` |
| `SCOPES` | Default `openid profile email` |
| `ADDR` | Optional; default `127.0.0.1:8080`; on conflict use `YUFU_APP_PORT` from `state.env` |

Read the secret only from the server environment. Never expose it to the frontend, commit it to the
repository, or print it in logs.

`register-app.sh` picks a free port starting at `8080` when `8080` is busy and records it as `YUFU_APP_PORT`
in `state.env`. Start the app with `ADDR=127.0.0.1:$YUFU_APP_PORT`; `REDIRECT_URI` must match the registered
value byte for byte.

## 2. `config.go`

```go
package main

import (
	"errors"
	"os"
	"strings"

	yufu "github.com/Bin-hy/yufu-sdk-go"
)

type Config struct {
	Issuer                string
	ClientID              string
	ClientSecret          string
	RedirectURI           string
	PostLogoutRedirectURI string
	Audience              string
	Addr                  string
	Scopes                []string
}

func LoadConfig() (Config, error) {
	cfg := Config{
		Issuer:                os.Getenv("ISSUER"),
		ClientID:              os.Getenv("CLIENT_ID"),
		ClientSecret:          os.Getenv("CLIENT_SECRET"),
		RedirectURI:           os.Getenv("REDIRECT_URI"),
		PostLogoutRedirectURI: os.Getenv("POST_LOGOUT_REDIRECT_URI"),
		Audience:              os.Getenv("AUDIENCE"),
		Addr:                  os.Getenv("ADDR"),
	}
	if cfg.Addr == "" {
		cfg.Addr = "127.0.0.1:8080"
	}
	if cfg.PostLogoutRedirectURI == "" {
		cfg.PostLogoutRedirectURI = cfg.RedirectURI
	}
	scopes := os.Getenv("SCOPES")
	if scopes == "" {
		scopes = "openid profile email"
	}
	cfg.Scopes = strings.Fields(scopes)
	if cfg.Issuer == "" || cfg.ClientID == "" || cfg.ClientSecret == "" ||
		cfg.RedirectURI == "" || cfg.Audience == "" {
		return Config{}, errors.New("ISSUER, CLIENT_ID, CLIENT_SECRET, REDIRECT_URI and AUDIENCE are required")
	}
	return cfg, nil
}

func NewSDK(cfg Config) (*yufu.Client, error) {
	return yufu.New(yufu.Config{
		Issuer:       cfg.Issuer,
		ClientID:     cfg.ClientID,
		ClientSecret: cfg.ClientSecret,
		RedirectURI:  cfg.RedirectURI,
		Scopes:       cfg.Scopes,
	})
}
```

## 3. `session.go`

```go
package main

import (
	"crypto/rand"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"sync"

	yufu "github.com/Bin-hy/yufu-sdk-go"
)

// pendingStore keeps in-flight authorization requests. Development only:
// a real deployment must persist state, nonce and verifier across instances.
type pendingStore struct {
	mu sync.Mutex
	m  map[string]*yufu.AuthorizationRequest
}

func newPendingStore() *pendingStore {
	return &pendingStore{m: make(map[string]*yufu.AuthorizationRequest)}
}

func (p *pendingStore) put(req *yufu.AuthorizationRequest) {
	p.mu.Lock()
	p.m[req.State] = req
	p.mu.Unlock()
}

func (p *pendingStore) take(state string) *yufu.AuthorizationRequest {
	p.mu.Lock()
	defer p.mu.Unlock()
	req := p.m[state]
	delete(p.m, state)
	return req
}

const sessionCookie = "demo_session"

type session struct {
	Subject string `json:"sub"`
	Name    string `json:"name,omitempty"`
}

// sessionStore keeps sessions on the server. The cookie carries only a random
// identifier, so rewriting the cookie cannot change the subject. Development
// only: share the store across instances and expire idle sessions in production.
type sessionStore struct {
	mu sync.Mutex
	m  map[string]session
}

func newSessionStore() *sessionStore {
	return &sessionStore{m: make(map[string]session)}
}

func (s *sessionStore) create(value session) (string, error) {
	var raw [32]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return "", err
	}
	id := base64.RawURLEncoding.EncodeToString(raw[:])
	s.mu.Lock()
	s.m[id] = value
	s.mu.Unlock()
	return id, nil
}

func (s *sessionStore) get(id string) (session, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()
	value, ok := s.m[id]
	return value, ok
}

func (s *sessionStore) delete(id string) {
	s.mu.Lock()
	delete(s.m, id)
	s.mu.Unlock()
}

func setSession(w http.ResponseWriter, store *sessionStore, value session) error {
	id, err := store.create(value)
	if err != nil {
		return err
	}
	http.SetCookie(w, &http.Cookie{
		Name:     sessionCookie,
		Value:    id,
		Path:     "/",
		HttpOnly: true,
		SameSite: http.SameSiteLaxMode,
	})
	return nil
}

func currentSession(r *http.Request, store *sessionStore) (session, bool) {
	cookie, err := r.Cookie(sessionCookie)
	if err != nil {
		return session{}, false
	}
	return store.get(cookie.Value)
}

func clearSession(w http.ResponseWriter, r *http.Request, store *sessionStore) {
	if cookie, err := r.Cookie(sessionCookie); err == nil {
		store.delete(cookie.Value)
	}
	http.SetCookie(w, &http.Cookie{
		Name: sessionCookie, Value: "", Path: "/", MaxAge: -1, HttpOnly: true,
	})
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
```

The cookie carries only an unguessable random ID; user data lives in the server-side `sessionStore`, so
tampering with the cookie only yields an invalid ID. The example uses an in-process map so it can run
directly; multi-instance deployments must use shared storage (Redis or a database), add expiry and
sign-out propagation, and set `Secure: true` under HTTPS.

## 4. `login.go`

```go
package main

import (
	"net/http"

	yufu "github.com/Bin-hy/yufu-sdk-go"
)

type App struct {
	cfg      Config
	client   *yufu.Client
	pending  *pendingStore
	sessions *sessionStore
}

// Login starts the authorization code + PKCE flow.
func (a *App) Login(w http.ResponseWriter, r *http.Request) {
	auth, err := a.client.AuthorizeURL(yufu.AuthorizeOptions{})
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	a.pending.put(auth)
	http.Redirect(w, r, auth.URL, http.StatusFound)
}

// Callback exchanges the code and verifies both tokens before creating a session.
func (a *App) Callback(w http.ResponseWriter, r *http.Request) {
	state := r.URL.Query().Get("state")
	code := r.URL.Query().Get("code")
	if state == "" || code == "" {
		http.Error(w, "missing state or code", http.StatusBadRequest)
		return
	}
	pending := a.pending.take(state)
	if pending == nil {
		http.Error(w, "unknown or replayed state", http.StatusBadRequest)
		return
	}
	tokens, err := a.client.ExchangeCode(r.Context(), code, pending.CodeVerifier)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	if _, err := a.client.VerifyIDToken(r.Context(), tokens.IDToken, pending.Nonce); err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	if _, err := a.client.VerifyAccessToken(r.Context(), tokens.AccessToken, a.cfg.Audience); err != nil {
		http.Error(w, err.Error(), http.StatusUnauthorized)
		return
	}
	profile, err := a.client.UserInfo(r.Context(), tokens.AccessToken)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	if err := setSession(w, a.sessions, session{Subject: profile.Subject, Name: profile.Name}); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	http.Redirect(w, r, "/api/me", http.StatusFound)
}
```

The order matters: exchange the code first, then verify the ID token (with the nonce), then verify the
access token (with the audience), and only then create the local session.

## 5. `logout.go`

```go
package main

import (
	"net/http"
)

func (a *App) Me(w http.ResponseWriter, r *http.Request) {
	value, ok := currentSession(r, a.sessions)
	if !ok {
		http.Error(w, "unauthorized", http.StatusUnauthorized)
		return
	}
	writeJSON(w, http.StatusOK, value)
}

func (a *App) Logout(w http.ResponseWriter, r *http.Request) {
	clearSession(w, r, a.sessions)
	http.Redirect(w, r, "/", http.StatusFound)
}

func Routes(a *App) http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if _, ok := currentSession(r, a.sessions); ok {
			http.Redirect(w, r, "/api/me", http.StatusFound)
			return
		}
		http.Redirect(w, r, "/login", http.StatusFound)
	})
	mux.HandleFunc("/login", a.Login)
	mux.HandleFunc("/callback", a.Callback)
	mux.HandleFunc("/logout", a.Logout)
	mux.HandleFunc("/api/me", a.Me)
	return mux
}
```

Local verification only requires the local session to be invalidated: after `/logout`, `/api/me` must
return `401` again.

yufu's central SSO session is ended from the console (`POST /api/v1/identity/logout`). In the loopback
build, the `/oidc/logout` confirmation page still validates the legacy `refresh_token` cookie; confirming
a central session directly returns `400 missing refresh token`, so the default implementation does not send
users to that page. If you need RP-initiated sign-out, generate the URL with the SDK's `EndSessionURL`.
The registration script already writes `post_logout_redirect_uri` into `redirect_uris`, so the confirmation
page itself opens correctly.

## 6. Wiring

```go
package main

import (
	"log"
	"net/http"
)

func main() {
	cfg, err := LoadConfig()
	if err != nil {
		log.Fatal(err)
	}
	client, err := NewSDK(cfg)
	if err != nil {
		log.Fatal(err)
	}
	defer client.Close()

	app := &App{cfg: cfg, client: client, pending: newPendingStore(), sessions: newSessionStore()}
	log.Printf("listening on http://%s (issuer %s)", cfg.Addr, cfg.Issuer)
	log.Fatal(http.ListenAndServe(cfg.Addr, Routes(app)))
}
```

## 7. Verification

1. `go build ./...` passes.
2. Start the app and visit `/login` at the `ADDR` address. After signing in at yufu, the browser returns to
   `/api/me` and shows a stable `sub`.
3. After visiting `/logout`, `/api/me` requires sign-in again.
4. Do not use email or username as the local user key; `iss + sub` is stable.

This code is for loopback verification only. Production deployments must replace the in-memory store with
shared session storage, add session expiry and sign-out propagation, and enable `Secure` under HTTPS.
Back-channel logout `jti` deduplication can be added to the same routes as needed.
