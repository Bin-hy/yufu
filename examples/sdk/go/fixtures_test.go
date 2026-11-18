package main

import (
	"context"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"

	yufu "github.com/Bin-hy/yufu-sdk-go"
)

type contractManifest struct {
	Version string           `json:"version"`
	Issuer  string           `json:"issuer"`
	Clock   int64            `json:"clock"`
	Flow    contractFlow     `json:"flow"`
	Files   contractFileList `json:"files"`
}

type contractFlow struct {
	State                 string         `json:"state"`
	Nonce                 string         `json:"nonce"`
	Code                  string         `json:"code"`
	CodeVerifier          string         `json:"code_verifier"`
	WrongCodeVerifier     string         `json:"wrong_code_verifier"`
	Client                contractClient `json:"client"`
	Reader                contractClient `json:"reader"`
	RedirectURI           string         `json:"redirect_uri"`
	PostLogoutRedirectURI string         `json:"post_logout_redirect_uri"`
	Audience              string         `json:"audience"`
	Scopes                []string       `json:"scopes"`
}

type contractClient struct {
	ClientID     string `json:"client_id"`
	ClientSecret string `json:"client_secret"`
}

type contractFileList struct {
	Discovery    string            `json:"discovery"`
	JWKS         string            `json:"jwks"`
	Endpoints    string            `json:"endpoints"`
	Expectations string            `json:"expectations"`
	Keys         map[string]string `json:"keys"`
}

type endpointSet struct {
	Endpoints []endpointSpec `json:"endpoints"`
}

type endpointSpec struct {
	Method  string         `json:"method"`
	Path    string         `json:"path"`
	Cases   []endpointCase `json:"cases"`
	Default responseSpec   `json:"default"`
}

type endpointCase struct {
	ID           string            `json:"id"`
	FormEquals   map[string]string `json:"form_equals"`
	FormPresent  []string          `json:"form_present"`
	HeadersMatch map[string]string `json:"headers_match"`
	Response     responseSpec      `json:"response"`
}

type responseSpec struct {
	Status   int               `json:"status"`
	Headers  map[string]string `json:"headers"`
	JSON     json.RawMessage   `json:"json"`
	BodyFile string            `json:"body_file"`
}

type expectationSet struct {
	AccessToken []tokenExpectation `json:"access_token"`
	IDToken     []tokenExpectation `json:"id_token"`
	LogoutToken []tokenExpectation `json:"logout_token"`
}

type tokenExpectation struct {
	ID       string `json:"id"`
	Token    string `json:"token"`
	Audience string `json:"audience"`
	Nonce    string `json:"nonce"`
	Expect   string `json:"expect"`
	Reason   string `json:"reason"`
}

type contractFixture struct {
	root         string
	manifest     contractManifest
	endpoints    endpointSet
	expectations expectationSet
}

func loadContractFixture(t *testing.T) *contractFixture {
	t.Helper()
	root := os.Getenv("YUFU_CONTRACT_FIXTURES")
	if root == "" {
		root = filepath.Join("..", "..", "..", "sdk", "contract-fixtures")
	}
	absolute, err := filepath.Abs(root)
	if err != nil {
		t.Fatal(err)
	}
	fixture := &contractFixture{root: absolute}
	fixture.loadJSON(t, "manifest.json", &fixture.manifest)
	for _, rel := range []string{
		fixture.manifest.Files.Discovery,
		fixture.manifest.Files.JWKS,
		fixture.manifest.Files.Endpoints,
		fixture.manifest.Files.Expectations,
	} {
		fixture.mustExist(t, rel)
	}
	for _, rel := range fixture.manifest.Files.Keys {
		fixture.mustExist(t, rel)
	}
	fixture.loadJSON(t, fixture.manifest.Files.Endpoints, &fixture.endpoints)
	fixture.loadJSON(t, fixture.manifest.Files.Expectations, &fixture.expectations)
	for _, expectation := range fixture.allExpectations() {
		fixture.mustExist(t, expectation.Token)
	}
	for _, endpoint := range fixture.endpoints.Endpoints {
		if endpoint.Default.BodyFile != "" {
			fixture.mustExist(t, endpoint.Default.BodyFile)
		}
		for _, item := range endpoint.Cases {
			if item.Response.BodyFile != "" {
				fixture.mustExist(t, item.Response.BodyFile)
			}
		}
	}
	return fixture
}

func (f *contractFixture) loadJSON(t *testing.T, rel string, target any) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(f.root, rel))
	if err != nil {
		t.Fatalf("contract fixture %s: %v", rel, err)
	}
	if err := json.Unmarshal(data, target); err != nil {
		t.Fatalf("contract fixture %s: %v", rel, err)
	}
}

func (f *contractFixture) mustExist(t *testing.T, rel string) {
	t.Helper()
	if _, err := os.Stat(filepath.Join(f.root, rel)); err != nil {
		t.Fatalf("contract fixture is missing %s: %v", rel, err)
	}
}

func (f *contractFixture) allExpectations() []tokenExpectation {
	result := make([]tokenExpectation, 0,
		len(f.expectations.AccessToken)+len(f.expectations.IDToken)+len(f.expectations.LogoutToken))
	result = append(result, f.expectations.AccessToken...)
	result = append(result, f.expectations.IDToken...)
	result = append(result, f.expectations.LogoutToken...)
	return result
}

func (f *contractFixture) now() time.Time {
	return time.Unix(f.manifest.Clock, 0).UTC()
}

func (f *contractFixture) readToken(t *testing.T, rel string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(f.root, rel))
	if err != nil {
		t.Fatalf("read token %s: %v", rel, err)
	}
	return strings.TrimSpace(string(data))
}

func (f *contractFixture) startServer(t *testing.T) {
	t.Helper()
	issuer, err := url.Parse(f.manifest.Issuer)
	if err != nil {
		t.Fatalf("invalid fixture issuer: %v", err)
	}
	listener, err := net.Listen("tcp", issuer.Host)
	if err != nil {
		t.Fatalf("fixture issuer %s is unavailable: %v (another consumer may be running)",
			f.manifest.Issuer, err)
	}
	mux := http.NewServeMux()
	mux.HandleFunc("/.well-known/openid-configuration", f.serveFile(f.manifest.Files.Discovery))
	mux.HandleFunc("/.well-known/jwks.json", f.serveFile(f.manifest.Files.JWKS))
	mux.HandleFunc("/", f.handleEndpoint)
	server := &http.Server{Handler: mux}
	go func() { _ = server.Serve(listener) }()
	t.Cleanup(func() {
		_ = server.Close()
		_ = listener.Close()
	})
}

func (f *contractFixture) serveFile(rel string) http.HandlerFunc {
	return func(w http.ResponseWriter, _ *http.Request) {
		data, err := os.ReadFile(filepath.Join(f.root, rel))
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(data)
	}
}

func (f *contractFixture) handleEndpoint(w http.ResponseWriter, r *http.Request) {
	for _, endpoint := range f.endpoints.Endpoints {
		if endpoint.Path != r.URL.Path {
			continue
		}
		if !strings.EqualFold(endpoint.Method, r.Method) {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		if err := r.ParseForm(); err != nil {
			http.Error(w, "invalid form", http.StatusBadRequest)
			return
		}
		for _, item := range endpoint.Cases {
			if matchesCase(r, item) {
				f.writeResponse(w, item.Response)
				return
			}
		}
		f.writeResponse(w, endpoint.Default)
		return
	}
	http.NotFound(w, r)
}

func matchesCase(r *http.Request, item endpointCase) bool {
	for key, want := range item.FormEquals {
		if r.Form.Get(key) != want {
			return false
		}
	}
	for _, key := range item.FormPresent {
		if _, exists := r.Form[key]; !exists {
			return false
		}
	}
	for key, pattern := range item.HeadersMatch {
		matched, err := regexp.MatchString(pattern, r.Header.Get(key))
		if err != nil || !matched {
			return false
		}
	}
	return true
}

func (f *contractFixture) writeResponse(w http.ResponseWriter, spec responseSpec) {
	for key, value := range spec.Headers {
		w.Header().Set(key, value)
	}
	var body []byte
	switch {
	case spec.BodyFile != "":
		data, err := os.ReadFile(filepath.Join(f.root, spec.BodyFile))
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		body = data
		if w.Header().Get("Content-Type") == "" {
			w.Header().Set("Content-Type", "application/json")
		}
	case len(spec.JSON) > 0:
		body = append(spec.JSON, '\n')
		if w.Header().Get("Content-Type") == "" {
			w.Header().Set("Content-Type", "application/json")
		}
	}
	w.WriteHeader(spec.Status)
	if len(body) > 0 {
		_, _ = w.Write(body)
	}
}

func (f *contractFixture) client(t *testing.T) *yufu.Client {
	t.Helper()
	client, err := yufu.New(yufu.Config{
		Issuer:       f.manifest.Issuer,
		ClientID:     f.manifest.Flow.Client.ClientID,
		ClientSecret: f.manifest.Flow.Client.ClientSecret,
		RedirectURI:  f.manifest.Flow.RedirectURI,
		Scopes:       f.manifest.Flow.Scopes,
		Now:          f.now,
	})
	if err != nil {
		t.Fatalf("new client: %v", err)
	}
	t.Cleanup(func() { _ = client.Close() })
	return client
}

func (f *contractFixture) reader(t *testing.T) *yufu.Client {
	t.Helper()
	client, err := yufu.New(yufu.Config{
		Issuer:       f.manifest.Issuer,
		ClientID:     f.manifest.Flow.Reader.ClientID,
		ClientSecret: f.manifest.Flow.Reader.ClientSecret,
		Now:          f.now,
	})
	if err != nil {
		t.Fatalf("new reader: %v", err)
	}
	t.Cleanup(func() { _ = client.Close() })
	return client
}

func TestContractDiscoveryAndJWKS(t *testing.T) {
	fixture := loadContractFixture(t)
	fixture.startServer(t)
	client := fixture.client(t)
	ctx := context.Background()

	discovery, err := client.Discover(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if discovery.Issuer != fixture.manifest.Issuer {
		t.Fatalf("discovery issuer = %q", discovery.Issuer)
	}
	if discovery.AuthorizationEndpoint != fixture.manifest.Issuer+"/oidc/authorize" {
		t.Fatalf("authorization endpoint = %q", discovery.AuthorizationEndpoint)
	}
	if !discovery.BackchannelLogoutSupported || !discovery.BackchannelLogoutSessionSupported {
		t.Fatal("discovery does not advertise back-channel logout")
	}

	claims, err := client.VerifyAccessToken(ctx,
		fixture.readToken(t, "tokens/access-token.jwt"),
		fixture.manifest.Flow.Audience)
	if err != nil {
		t.Fatalf("verify access token with fixture JWKS: %v", err)
	}
	if claims.Subject != "42" || claims.TokenUse != "user_session" {
		t.Fatalf("unexpected access token claims: %+v", claims)
	}
}

func TestContractAuthorizationCodeFlow(t *testing.T) {
	fixture := loadContractFixture(t)
	fixture.startServer(t)
	client := fixture.client(t)
	reader := fixture.reader(t)
	flow := fixture.manifest.Flow
	ctx := context.Background()

	authorization, err := client.AuthorizeURL(yufu.AuthorizeOptions{
		State: flow.State,
		Nonce: flow.Nonce,
	})
	if err != nil {
		t.Fatal(err)
	}
	parsed, err := url.Parse(authorization.URL)
	if err != nil {
		t.Fatal(err)
	}
	if got := parsed.Query().Get("state"); got != flow.State {
		t.Fatalf("state = %q", got)
	}
	if got := parsed.Query().Get("nonce"); got != flow.Nonce {
		t.Fatalf("nonce = %q", got)
	}
	if got := parsed.Query().Get("code_challenge_method"); got != "S256" {
		t.Fatalf("code_challenge_method = %q", got)
	}
	digest := sha256.Sum256([]byte(authorization.CodeVerifier))
	if want := base64.RawURLEncoding.EncodeToString(digest[:]); authorization.CodeChallenge != want {
		t.Fatalf("code_challenge = %q, want S256(verifier) = %q", authorization.CodeChallenge, want)
	}

	// The SDK generates its own verifier; the HTTP contract uses the fixture's
	// canonical verifier so the signed ID token and nonce stay static.
	tokens, err := client.ExchangeCode(ctx, flow.Code, flow.CodeVerifier)
	if err != nil {
		t.Fatalf("exchange code: %v", err)
	}
	if tokens.RefreshToken != "refresh-1" || tokens.ExpiresIn != 300 {
		t.Fatalf("unexpected token set: %+v", tokens)
	}

	idClaims, err := client.VerifyIDToken(ctx, tokens.IDToken, flow.Nonce)
	if err != nil {
		t.Fatalf("verify ID token: %v", err)
	}
	if idClaims.Subject != "42" || idClaims.SID != "sid-1" {
		t.Fatalf("unexpected ID token claims: %+v", idClaims)
	}

	accessClaims, err := client.VerifyAccessToken(ctx, tokens.AccessToken, flow.Audience)
	if err != nil {
		t.Fatalf("verify access token: %v", err)
	}
	if accessClaims.Audience != flow.Audience || accessClaims.TokenUse != "user_session" {
		t.Fatalf("unexpected access token claims: %+v", accessClaims)
	}

	info, err := client.UserInfo(ctx, tokens.AccessToken)
	if err != nil {
		t.Fatalf("userinfo: %v", err)
	}
	if info.Subject != "42" || info.PreferredUsername != "alice" {
		t.Fatalf("unexpected userinfo: %+v", info)
	}

	introspection, err := reader.Introspect(ctx, tokens.AccessToken, yufu.TokenTypeAccessToken)
	if err != nil {
		t.Fatalf("introspect: %v", err)
	}
	if !introspection.Active || introspection.ClientID != "web" {
		t.Fatalf("unexpected introspection: %+v", introspection)
	}

	if err := client.Revoke(ctx, tokens.AccessToken); err != nil {
		t.Fatalf("revoke: %v", err)
	}
	if err := client.Revoke(ctx, tokens.AccessToken); err != nil {
		t.Fatalf("replayed revoke must be idempotent: %v", err)
	}

	endSession, err := client.EndSessionURL(ctx, yufu.EndSessionOptions{
		PostLogoutRedirectURI: flow.PostLogoutRedirectURI,
	})
	if err != nil {
		t.Fatal(err)
	}
	parsed, err = url.Parse(endSession)
	if err != nil {
		t.Fatal(err)
	}
	if parsed.Query().Get("client_id") != flow.Client.ClientID {
		t.Fatalf("end-session client_id = %q", parsed.Query().Get("client_id"))
	}
	if parsed.Query().Get("post_logout_redirect_uri") != flow.PostLogoutRedirectURI {
		t.Fatalf("end-session post_logout_redirect_uri = %q", parsed.Query().Get("post_logout_redirect_uri"))
	}
}

func TestContractRefresh(t *testing.T) {
	fixture := loadContractFixture(t)
	fixture.startServer(t)
	client := fixture.client(t)
	ctx := context.Background()

	tokens, err := client.Refresh(ctx, "refresh-1")
	if err != nil {
		t.Fatalf("refresh: %v", err)
	}
	if tokens.RefreshToken != "refresh-2" {
		t.Fatalf("refresh token = %q", tokens.RefreshToken)
	}
	claims, err := client.VerifyAccessToken(ctx, tokens.AccessToken, fixture.manifest.Flow.Audience)
	if err != nil {
		t.Fatalf("verify refreshed access token: %v", err)
	}
	if claims.JTI != "jti-2" {
		t.Fatalf("refreshed access token jti = %q", claims.JTI)
	}
}

func TestContractEndpointErrors(t *testing.T) {
	fixture := loadContractFixture(t)
	fixture.startServer(t)
	client := fixture.client(t)
	flow := fixture.manifest.Flow
	ctx := context.Background()

	_, err := client.ExchangeCode(ctx, flow.Code, flow.WrongCodeVerifier)
	if !yufu.IsErrorCode(err, "invalid_grant") {
		t.Fatalf("wrong PKCE error = %v, want invalid_grant", err)
	}

	status, body := postForm(t, fixture.manifest.Issuer+"/oidc/token",
		url.Values{"grant_type": {"client_credentials"}})
	if status != http.StatusBadRequest {
		t.Fatalf("unsupported grant status = %d", status)
	}
	if got := oauthError(t, body); got != "unsupported_grant_type" {
		t.Fatalf("unsupported grant error = %q", got)
	}

	response, err := http.Get(fixture.manifest.Issuer + "/oidc/userinfo")
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusUnauthorized {
		t.Fatalf("userinfo without bearer status = %d", response.StatusCode)
	}

	status, body = postForm(t, fixture.manifest.Issuer+"/oidc/introspect", url.Values{
		"token":         {"any"},
		"client_id":     {flow.Reader.ClientID},
		"client_secret": {"wrong"},
	})
	if status != http.StatusUnauthorized {
		t.Fatalf("introspection bad credentials status = %d", status)
	}
	if got := oauthError(t, body); got != "invalid_client" {
		t.Fatalf("introspection error = %q", got)
	}
}

func TestContractNegativeTokens(t *testing.T) {
	fixture := loadContractFixture(t)
	fixture.startServer(t)
	client := fixture.client(t)
	flow := fixture.manifest.Flow
	ctx := context.Background()

	for _, item := range fixture.expectations.AccessToken {
		t.Run(item.ID, func(t *testing.T) {
			_, err := client.VerifyAccessToken(ctx, fixture.readToken(t, item.Token), item.Audience)
			assertExpectation(t, item, err)
		})
	}
	for _, item := range fixture.expectations.IDToken {
		t.Run(item.ID, func(t *testing.T) {
			_, err := client.VerifyIDToken(ctx, fixture.readToken(t, item.Token), item.Nonce)
			assertExpectation(t, item, err)
		})
	}
	for _, item := range fixture.expectations.LogoutToken {
		t.Run(item.ID, func(t *testing.T) {
			_, err := client.VerifyLogoutToken(ctx, fixture.readToken(t, item.Token))
			assertExpectation(t, item, err)
			if err == nil {
				return
			}
		})
	}

	// Sanity: the valid access token is accepted with the fixture audience.
	if _, err := client.VerifyAccessToken(ctx,
		fixture.readToken(t, "tokens/access-token.jwt"), flow.Audience); err != nil {
		t.Fatalf("valid access token rejected: %v", err)
	}
}

func assertExpectation(t *testing.T, item tokenExpectation, err error) {
	t.Helper()
	switch item.Expect {
	case "accept":
		if err != nil {
			t.Fatalf("expected accept, got %v", err)
		}
	case "reject":
		if err == nil {
			t.Fatal("expected reject, got nil")
		}
		if got := rejectionReason(err); got != item.Reason {
			t.Fatalf("rejection reason = %q, want %q (error: %v)", got, item.Reason, err)
		}
	default:
		t.Fatalf("unknown expectation %q", item.Expect)
	}
}

func rejectionReason(err error) string {
	var typed *yufu.Error
	if !errors.As(err, &typed) {
		return ""
	}
	description := typed.Description
	switch {
	case strings.Contains(description, "token is expired"):
		return "expired"
	case strings.Contains(description, "signature verification failed"):
		return "bad_signature"
	case strings.Contains(description, "JWKS has no key"):
		return "unknown_kid"
	case strings.Contains(description, "unexpected token typ"):
		return "typ_mismatch"
	case strings.Contains(description, "issuer does not match"):
		return "issuer_mismatch"
	case strings.Contains(description, "audience does not match"):
		return "audience_mismatch"
	case strings.Contains(description, "missing events"):
		return "missing_events"
	case strings.Contains(description, "must not contain nonce"):
		return "nonce_present"
	case strings.Contains(description, "lifetime must be at most"):
		return "lifetime_too_long"
	default:
		return ""
	}
}

func postForm(t *testing.T, endpoint string, values url.Values) (int, []byte) {
	t.Helper()
	response, err := http.PostForm(endpoint, values)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	body, err := io.ReadAll(response.Body)
	if err != nil {
		t.Fatal(err)
	}
	return response.StatusCode, body
}

func oauthError(t *testing.T, body []byte) string {
	t.Helper()
	var payload struct {
		Error string `json:"error"`
	}
	if err := json.Unmarshal(body, &payload); err != nil {
		t.Fatalf("decode oauth error %q: %v", body, err)
	}
	return payload.Error
}