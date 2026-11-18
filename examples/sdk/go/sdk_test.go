package main

import (
	"context"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"
)

func TestExampleBackchannelLogout(t *testing.T) {
	fixture := loadContractFixture(t)
	fixture.startServer(t)
	client := fixture.client(t)
	ctx := context.Background()

	token := fixture.readToken(t, "tokens/logout-token.jwt")
	claims, err := client.VerifyLogoutToken(ctx, token)
	if err != nil {
		t.Fatalf("verify logout token: %v", err)
	}
	if claims.JTI != "jti-3" || claims.SID != "sid-1" {
		t.Fatalf("unexpected logout claims: %+v", claims)
	}

	app := &demo{
		client:  client,
		seenJTI: make(map[string]time.Time),
	}
	deliver := func(logoutToken string) int {
		recorder := httptest.NewRecorder()
		request := httptest.NewRequest(http.MethodPost, "/oidc/backchannel-logout",
			strings.NewReader(url.Values{"logout_token": {logoutToken}}.Encode()))
		request.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		app.backchannelLogout(recorder, request)
		return recorder.Code
	}

	for i := 0; i < 2; i++ {
		if got := deliver(token); got != http.StatusOK {
			t.Fatalf("delivery %d status = %d", i+1, got)
		}
	}
	if got := deliver("not-a-token"); got != http.StatusBadRequest {
		t.Fatalf("invalid logout token status = %d", got)
	}
}