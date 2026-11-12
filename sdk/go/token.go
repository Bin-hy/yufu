package cobotusr

import (
	"context"
	"errors"
	"fmt"
	"net/url"
	"strings"
	"time"
)

// TokenSet is the result of a code exchange or refresh.
type TokenSet struct {
	AccessToken  string
	IDToken      string
	RefreshToken string
	TokenType    string
	ExpiresIn    int64
	Scope        string
	ExpiresAt    time.Time
}

type tokenResponse struct {
	AccessToken  string `json:"access_token"`
	IDToken      string `json:"id_token"`
	RefreshToken string `json:"refresh_token"`
	TokenType    string `json:"token_type"`
	ExpiresIn    int64  `json:"expires_in"`
	Scope        string `json:"scope"`
}

func (c *Client) tokenSet(response tokenResponse) (*TokenSet, error) {
	if response.AccessToken == "" {
		return nil, &Error{Code: "invalid_response", Description: "token response is missing access_token"}
	}
	if response.TokenType == "" {
		response.TokenType = "Bearer"
	}
	if !strings.EqualFold(response.TokenType, "Bearer") {
		return nil, &Error{
			Code:        "invalid_response",
			Description: fmt.Sprintf("unsupported token_type %q", response.TokenType),
		}
	}
	tokens := &TokenSet{
		AccessToken:  response.AccessToken,
		IDToken:      response.IDToken,
		RefreshToken: response.RefreshToken,
		TokenType:    response.TokenType,
		ExpiresIn:    response.ExpiresIn,
		Scope:        response.Scope,
	}
	if response.ExpiresIn > 0 {
		tokens.ExpiresAt = c.now().Add(time.Duration(response.ExpiresIn) * time.Second)
	}
	return tokens, nil
}

// Refresh redeems a refresh token. Concurrent calls with the same refresh
// token are coalesced into one server request.
func (c *Client) Refresh(ctx context.Context, refreshToken string) (*TokenSet, error) {
	if strings.TrimSpace(refreshToken) == "" {
		return nil, errors.New("cobotusr: refresh token is required")
	}
	c.refreshMu.Lock()
	if call, ok := c.refreshes[refreshToken]; ok {
		c.refreshMu.Unlock()
		select {
		case <-call.done:
			return call.tokens, call.err
		case <-ctx.Done():
			return nil, &Error{Code: "context_canceled", Err: ctx.Err()}
		}
	}
	call := &refreshCall{done: make(chan struct{})}
	c.refreshes[refreshToken] = call
	c.refreshMu.Unlock()

	tokens, err := c.refresh(ctx, refreshToken)
	call.tokens, call.err = tokens, err
	close(call.done)
	c.refreshMu.Lock()
	delete(c.refreshes, refreshToken)
	c.refreshMu.Unlock()
	return tokens, err
}

func (c *Client) refresh(ctx context.Context, refreshToken string) (*TokenSet, error) {
	discovery, err := c.Discover(ctx)
	if err != nil {
		return nil, err
	}
	form := url.Values{}
	form.Set("grant_type", "refresh_token")
	form.Set("refresh_token", refreshToken)
	auth := c.auth()
	c.applyAuth(form, auth)
	var response tokenResponse
	if err := c.doForm(ctx, discovery.TokenEndpoint, form, auth, &response); err != nil {
		return nil, err
	}
	tokens, err := c.tokenSet(response)
	if err != nil {
		return nil, err
	}
	if tokens.RefreshToken == "" {
		tokens.RefreshToken = refreshToken
	}
	return tokens, nil
}
