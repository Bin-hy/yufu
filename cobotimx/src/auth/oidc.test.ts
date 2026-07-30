// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearAuth,
  createAuthorizationUrl,
  getAccessToken,
  hasExplicitlySignedOut,
  handleCallback,
  refreshAccessToken,
  revokeAndClearAuth,
} from "./oidc";

const ACCESS_KEY = "cobotimx.access_token";
const REFRESH_KEY = "cobotimx.refresh_token";
const EXPIRY_KEY = "cobotimx.expires_at";
const VERIFIER_KEY = "cobotimx.pkce_verifier";
const STATE_KEY = "cobotimx.oauth_state";
const DESTINATION_KEY = "cobotimx.post_login_path";

function tokenResponse(tokens: Record<string, unknown>) {
  return new Response(JSON.stringify(tokens), { status: 200 });
}

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  vi.restoreAllMocks();
  vi.stubGlobal("crypto", {
    getRandomValues: (bytes: Uint8Array) => {
      bytes.fill(7);
      return bytes;
    },
    subtle: {
      digest: vi.fn(async () => new Uint8Array([1, 2, 3]).buffer),
    },
  });
});

describe("IM OIDC PKCE flow", () => {
  it("builds an S256 authorization request and stores transient state", async () => {
    const url = new URL(await createAuthorizationUrl("/chat?conversation=7"));

    expect(url.origin).toBe("http://127.0.0.1:4000");
    expect(url.pathname).toBe("/oidc/authorize");
    expect(url.searchParams.get("response_type")).toBe("code");
    expect(url.searchParams.get("client_id")).toBe("cobotimx");
    expect(url.searchParams.get("scope")).toBe("openid profile email");
    expect(url.searchParams.get("code_challenge_method")).toBe("S256");
    expect(url.searchParams.get("code_challenge")).toBe("AQID");
    expect(sessionStorage.getItem(VERIFIER_KEY)).toBeTruthy();
    expect(sessionStorage.getItem(STATE_KEY)).toBe(url.searchParams.get("state"));
    expect(sessionStorage.getItem(DESTINATION_KEY)).toBe("/chat?conversation=7");
  });

  it("rejects a callback with a mismatched state without storing tokens", async () => {
    sessionStorage.setItem(STATE_KEY, "expected-state");
    sessionStorage.setItem(VERIFIER_KEY, "verifier");
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    await expect(handleCallback("code", "wrong-state")).rejects.toThrow("state mismatch");
    expect(fetchMock).not.toHaveBeenCalled();
    expect(localStorage.getItem(ACCESS_KEY)).toBeNull();
  });

  it("rejects a callback without its PKCE verifier without storing tokens", async () => {
    sessionStorage.setItem(STATE_KEY, "expected-state");
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    await expect(handleCallback("code", "expected-state")).rejects.toThrow("Missing PKCE verifier");
    expect(fetchMock).not.toHaveBeenCalled();
    expect(localStorage.getItem(ACCESS_KEY)).toBeNull();
  });

  it("exchanges an authorization code, clears PKCE state, and restores the destination", async () => {
    sessionStorage.setItem(STATE_KEY, "expected-state");
    sessionStorage.setItem(VERIFIER_KEY, "verifier");
    sessionStorage.setItem(DESTINATION_KEY, "/chat");
    const fetchMock = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => tokenResponse({
      access_token: "access-token",
      refresh_token: "refresh-token",
      expires_in: 3600,
    }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(handleCallback("authorization-code", "expected-state")).resolves.toBe("/chat");
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:4000/oidc/token",
      expect.objectContaining({ method: "POST" }),
    );
    const request = fetchMock.mock.calls[0][1] as RequestInit;
    expect(new URLSearchParams(request.body as string).get("code_verifier")).toBe("verifier");
    expect(localStorage.getItem(ACCESS_KEY)).toBe("access-token");
    expect(localStorage.getItem(REFRESH_KEY)).toBe("refresh-token");
    expect(sessionStorage.getItem(VERIFIER_KEY)).toBeNull();
    expect(sessionStorage.getItem(STATE_KEY)).toBeNull();
    expect(sessionStorage.getItem(DESTINATION_KEY)).toBeNull();
  });

  it("exchanges a callback authorization code only once when invoked concurrently", async () => {
    sessionStorage.setItem(STATE_KEY, "expected-state");
    sessionStorage.setItem(VERIFIER_KEY, "verifier");
    const fetchMock = vi.fn(async () => tokenResponse({
      access_token: "access-token",
      expires_in: 3600,
    }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      Promise.all([
        handleCallback("authorization-code", "expected-state"),
        handleCallback("authorization-code", "expected-state"),
      ]),
    ).resolves.toEqual(["/chat", "/chat"]);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("does not store tokens when the token endpoint rejects the authorization code", async () => {
    sessionStorage.setItem(STATE_KEY, "expected-state");
    sessionStorage.setItem(VERIFIER_KEY, "verifier");
    vi.stubGlobal("fetch", vi.fn(async () => new Response("invalid code", { status: 400 })));

    await expect(handleCallback("authorization-code", "expected-state")).rejects.toThrow("Token exchange failed: 400 (invalid code)");
    expect(localStorage.getItem(ACCESS_KEY)).toBeNull();
    expect(localStorage.getItem(REFRESH_KEY)).toBeNull();
  });

  it("refreshes the access token and clears all tokens when refresh fails", async () => {
    localStorage.setItem(REFRESH_KEY, "refresh-token");
    vi.stubGlobal("fetch", vi.fn(async () => tokenResponse({ access_token: "new-access", expires_in: 3600 })));

    await expect(refreshAccessToken()).resolves.toBe("new-access");
    expect(getAccessToken()).toBe("new-access");

    vi.stubGlobal("fetch", vi.fn(async () => new Response("invalid", { status: 401 })));
    await expect(refreshAccessToken()).resolves.toBeNull();
    expect(localStorage.getItem(ACCESS_KEY)).toBeNull();
    expect(localStorage.getItem(REFRESH_KEY)).toBeNull();
    expect(localStorage.getItem(EXPIRY_KEY)).toBeNull();
  });

  it("keeps existing tokens when refresh fails because of a network error", async () => {
    localStorage.setItem(ACCESS_KEY, "old-access");
    localStorage.setItem(REFRESH_KEY, "refresh-token");
    localStorage.setItem(EXPIRY_KEY, String(Date.now() + 3_600_000));
    vi.stubGlobal("fetch", vi.fn(async () => {
      throw new TypeError("network error");
    }));

    await expect(refreshAccessToken()).resolves.toBeNull();

    expect(localStorage.getItem(ACCESS_KEY)).toBe("old-access");
    expect(localStorage.getItem(REFRESH_KEY)).toBe("refresh-token");
    expect(localStorage.getItem(EXPIRY_KEY)).toBeTruthy();
  });

  it("revokes the refresh token and clears local authentication", async () => {
    localStorage.setItem(ACCESS_KEY, "access-token");
    localStorage.setItem(REFRESH_KEY, "refresh-token");
    localStorage.setItem(EXPIRY_KEY, String(Date.now() + 3_600_000));
    const fetchMock = vi.fn(async () => new Response(null, { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);

    await revokeAndClearAuth();

    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:4000/oidc/revoke",
      expect.objectContaining({ method: "POST" }),
    );
    expect(localStorage.getItem(ACCESS_KEY)).toBeNull();
    expect(localStorage.getItem(REFRESH_KEY)).toBeNull();
    expect(hasExplicitlySignedOut()).toBe(true);
  });

  it("clears the explicit-sign-out marker when the user starts a new authorization flow", async () => {
    localStorage.setItem("cobotimx.explicitly_signed_out", "true");

    await createAuthorizationUrl("/chat");

    expect(hasExplicitlySignedOut()).toBe(false);
  });
});

afterEach(() => {
  clearAuth();
});
