package cobotusr

import (
	"time"

	"github.com/Bin-hy/cobot-sdk-go/internal/jose"
)

// AccessTokenClaims is the validated access-token profile exposed to resource
// servers. Managed tokens use typ=at+jwt and aud=the application audience.
type AccessTokenClaims struct {
	Issuer      string
	Subject     string
	Audience    string
	Audiences   []string
	ExpiresAt   time.Time
	IssuedAt    time.Time
	NotBefore   time.Time
	JTI         string
	Scope       string
	TokenUse    string
	Application string
	SessionID   string
	ClientID    string
	Roles       []string
	Permissions []string
	Raw         map[string]any
}

// IDTokenClaims is the validated OpenID Connect ID Token profile.
type IDTokenClaims struct {
	Issuer            string
	Subject           string
	Audience          string
	Audiences         []string
	ExpiresAt         time.Time
	IssuedAt          time.Time
	Nonce             string
	AuthTime          time.Time
	SID               string
	Name              string
	PreferredUsername string
	Email             string
	EmailVerified     *bool
	AtHash            string
	Raw               map[string]any
}

// LogoutTokenClaims is a validated OIDC back-channel logout token.
type LogoutTokenClaims struct {
	Issuer    string
	Audience  string
	Subject   string
	SID       string
	JTI       string
	IssuedAt  time.Time
	ExpiresAt time.Time
	Events    map[string]any
	Raw       map[string]any
}

func accessTokenClaimsFrom(raw map[string]any) (*AccessTokenClaims, error) {
	subject, ok := jose.StringClaim(raw, "sub")
	if !ok || subject == "" {
		return nil, &Error{Code: "invalid_token", Description: "access token is missing sub"}
	}
	audiences, err := jose.AudienceClaim(raw)
	if err != nil {
		return nil, &Error{Code: "invalid_token", Description: err.Error(), Err: err}
	}
	expiresAt, ok := jose.NumberClaim(raw, "exp")
	if !ok {
		return nil, &Error{Code: "invalid_token", Description: "access token is missing exp"}
	}
	claims := &AccessTokenClaims{
		Subject:   subject,
		Audience:  audiences[0],
		Audiences: audiences,
		ExpiresAt: time.Unix(expiresAt, 0).UTC(),
		Raw:       raw,
	}
	claims.Issuer, _ = jose.StringClaim(raw, "iss")
	claims.JTI, _ = jose.StringClaim(raw, "jti")
	claims.Scope, _ = jose.StringClaim(raw, "scope")
	claims.TokenUse, _ = jose.StringClaim(raw, "token_use")
	claims.Application, _ = jose.StringClaim(raw, "application")
	claims.SessionID, _ = jose.StringClaim(raw, "session_id")
	claims.ClientID, _ = jose.StringClaim(raw, "client_id")
	claims.Roles = jose.StringSliceClaim(raw, "roles")
	claims.Permissions = jose.StringSliceClaim(raw, "permissions")
	if issuedAt, ok := jose.NumberClaim(raw, "iat"); ok {
		claims.IssuedAt = time.Unix(issuedAt, 0).UTC()
	}
	if notBefore, ok := jose.NumberClaim(raw, "nbf"); ok {
		claims.NotBefore = time.Unix(notBefore, 0).UTC()
	}
	return claims, nil
}

func idTokenClaimsFrom(raw map[string]any) (*IDTokenClaims, error) {
	subject, ok := jose.StringClaim(raw, "sub")
	if !ok || subject == "" {
		return nil, &Error{Code: "invalid_token", Description: "ID token is missing sub"}
	}
	audiences, err := jose.AudienceClaim(raw)
	if err != nil {
		return nil, &Error{Code: "invalid_token", Description: err.Error(), Err: err}
	}
	expiresAt, ok := jose.NumberClaim(raw, "exp")
	if !ok {
		return nil, &Error{Code: "invalid_token", Description: "ID token is missing exp"}
	}
	claims := &IDTokenClaims{
		Subject:   subject,
		Audience:  audiences[0],
		Audiences: audiences,
		ExpiresAt: time.Unix(expiresAt, 0).UTC(),
		Raw:       raw,
	}
	claims.Issuer, _ = jose.StringClaim(raw, "iss")
	claims.Nonce, _ = jose.StringClaim(raw, "nonce")
	claims.SID, _ = jose.StringClaim(raw, "sid")
	claims.Name, _ = jose.StringClaim(raw, "name")
	claims.PreferredUsername, _ = jose.StringClaim(raw, "preferred_username")
	claims.Email, _ = jose.StringClaim(raw, "email")
	claims.AtHash, _ = jose.StringClaim(raw, "at_hash")
	if issuedAt, ok := jose.NumberClaim(raw, "iat"); ok {
		claims.IssuedAt = time.Unix(issuedAt, 0).UTC()
	}
	if authTime, ok := jose.NumberClaim(raw, "auth_time"); ok {
		claims.AuthTime = time.Unix(authTime, 0).UTC()
	}
	if emailVerified, ok := jose.BoolClaim(raw, "email_verified"); ok {
		claims.EmailVerified = &emailVerified
	}
	return claims, nil
}
