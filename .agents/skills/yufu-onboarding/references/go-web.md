# Go Web 后端接入（授权码 + PKCE）

适用于：需要让用户通过 yufu 登录、并在服务端持有会话的 Go Web 服务。
注册时使用 `--mode web`（`client_kind = server_web`，有 client secret）。

## 1. 拿到配置

| 环境变量 | 来源 |
| --- | --- |
| `ISSUER` | `state.env` 的 `YUFU_ISSUER` |
| `CLIENT_ID` | `state.env` 的 `YUFU_CLIENT_ID` |
| `CLIENT_SECRET` | `state.env` 的 `YUFU_CLIENT_SECRET_FILE` 文件内容 |
| `REDIRECT_URI` | 注册时的 `YUFU_REDIRECT_URI`，必须逐字符一致 |
| `AUDIENCE` | `state.env` 的 `YUFU_AUDIENCE` |
| `SCOPES` | 默认 `openid profile email` |
| `ADDR` | 可选；默认 `127.0.0.1:8080`，冲突时用 `state.env` 的 `YUFU_APP_PORT` |

secret 只从服务端环境读取，不进入前端、不写进仓库、不打印到日志。

`register-app.sh` 在 `8080` 被占用时会从 `8080` 起选择空闲端口，并写入 `state.env` 的 `YUFU_APP_PORT`。
启动应用时设置 `ADDR=127.0.0.1:$YUFU_APP_PORT`；`REDIRECT_URI` 必须与注册值逐字符一致。

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

cookie 里只有不可猜测的随机 ID，用户资料保存在服务端 `sessionStore`，篡改 cookie 只能得到一个无效 ID。
示例使用进程内 map 以便直接运行；多实例部署要换成共享存储（Redis 或数据库），补上过期时间与
注销联动，并在 HTTPS 下追加 `Secure: true`。

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

关键顺序不能调换：先兑换 code，再校验 ID Token（必须带 nonce），再校验 access token
（必须带 audience），最后才建立本地会话。

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

本机验收只要求本地会话失效：`/logout` 后访问 `/api/me` 必须重新返回 `401`。

yufu 的中心 SSO 会话由控制台退出（`POST /api/v1/identity/logout`）。本机版本的
`/oidc/logout` 确认页仍按 legacy `refresh_token` cookie 校验，中心会话直接确认会得到
`400 missing refresh token`，因此默认实现不把用户送去该页面。需要 RP 发起退出时，
可以用 SDK 的 `EndSessionURL` 生成地址；注册脚本已把 `post_logout_redirect_uri`
一并写入 `redirect_uris`，所以确认页本身可以正常打开。

## 6. 接线

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

## 7. 验证

1. `go build ./...` 通过。
2. 启动后访问 `/login`（`ADDR` 指定的地址），应在 yufu 完成登录并回到 `/api/me`，看到稳定 `sub`。
3. 访问 `/logout` 后，再访问 `/api/me` 应重新要求登录。
4. 不要用 email 或 username 作为本地用户主键；`iss + sub` 才稳定。

本节代码仅用于本机验收；生产部署必须替换为共享会话存储、补上会话过期与注销联动，并在 HTTPS 下启用 `Secure`。back-channel logout 的 `jti` 去重逻辑可在同一路由中按需补充。