export interface OidcTokens {
  access_token: string;
  refresh_token?: string;
  expires_in?: number;
}

export interface UserInfo {
  sub: number | string;
  preferred_username?: string;
  name?: string;
  nickname?: string;
  email?: string;
}

const STORAGE_KEYS = {
  accessToken: "cobotimx.access_token",
  refreshToken: "cobotimx.refresh_token",
  expiresAt: "cobotimx.expires_at",
  pkceVerifier: "cobotimx.pkce_verifier",
  oauthState: "cobotimx.oauth_state",
  postLoginPath: "cobotimx.post_login_path",
  explicitlySignedOut: "cobotimx.explicitly_signed_out",
} as const;

function readOidcIssuer(): string {
  const issuer = import.meta.env.VITE_OIDC_ISSUER;
  if (!issuer && import.meta.env.PROD) {
    throw new Error("VITE_OIDC_ISSUER is required for production builds");
  }
  return (issuer ?? "http://127.0.0.1:4000").replace(/\/$/, "");
}

// Resolved lazily so importing this module never throws; a production build
// missing VITE_OIDC_ISSUER fails with a clear error on first use instead of
// silently falling back to a loopback address.
let cachedOidcIssuer: string | null = null;
function oidcIssuer(): string {
  if (!cachedOidcIssuer) cachedOidcIssuer = readOidcIssuer();
  return cachedOidcIssuer;
}

const CLIENT_ID = import.meta.env.VITE_OIDC_CLIENT_ID ?? "cobotimx";
const SCOPE = "openid profile email";
const EXPIRY_SKEW_MS = 30_000;

let refreshInFlight: Promise<RefreshTokenResult> | null = null;
let callbackInFlight: {
  code: string;
  state: string;
  promise: Promise<string>;
} | null = null;
let completedCallback: { code: string; state: string; destination: string } | null = null;

function redirectUri(): string {
  return import.meta.env.VITE_OIDC_REDIRECT_URI ?? `${window.location.origin}/oauth/callback`;
}

function base64UrlEncode(input: ArrayBuffer | Uint8Array): string {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  let value = "";
  for (const byte of bytes) value += String.fromCharCode(byte);
  return btoa(value).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function randomBase64Url(byteLength: number): string {
  const bytes = new Uint8Array(byteLength);
  crypto.getRandomValues(bytes);
  return base64UrlEncode(bytes);
}

async function sha256(value: string): Promise<ArrayBuffer> {
  return crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
}

function storeTokens(tokens: OidcTokens): void {
  localStorage.setItem(STORAGE_KEYS.accessToken, tokens.access_token);
  if (tokens.refresh_token) localStorage.setItem(STORAGE_KEYS.refreshToken, tokens.refresh_token);
  const expiresIn = tokens.expires_in ?? 3600;
  localStorage.setItem(STORAGE_KEYS.expiresAt, String(Date.now() + expiresIn * 1000));
}

function clearExpiredAccessToken(): void {
  localStorage.removeItem(STORAGE_KEYS.accessToken);
  localStorage.removeItem(STORAGE_KEYS.expiresAt);
}

export function getAccessToken(): string | null {
  const token = localStorage.getItem(STORAGE_KEYS.accessToken);
  if (!token) return null;

  const expiresAt = Number(localStorage.getItem(STORAGE_KEYS.expiresAt) ?? 0);
  if (expiresAt && Date.now() >= expiresAt - EXPIRY_SKEW_MS) {
    clearExpiredAccessToken();
    return null;
  }
  return token;
}

export function getRefreshToken(): string | null {
  return localStorage.getItem(STORAGE_KEYS.refreshToken);
}

export function hasExplicitlySignedOut(): boolean {
  return localStorage.getItem(STORAGE_KEYS.explicitlySignedOut) === "true";
}

export function clearAuth(): void {
  callbackInFlight = null;
  completedCallback = null;
  localStorage.removeItem(STORAGE_KEYS.accessToken);
  localStorage.removeItem(STORAGE_KEYS.refreshToken);
  localStorage.removeItem(STORAGE_KEYS.expiresAt);
  sessionStorage.removeItem(STORAGE_KEYS.pkceVerifier);
  sessionStorage.removeItem(STORAGE_KEYS.oauthState);
  sessionStorage.removeItem(STORAGE_KEYS.postLoginPath);
}

export async function createAuthorizationUrl(currentPath?: string): Promise<string> {
  callbackInFlight = null;
  completedCallback = null;
  localStorage.removeItem(STORAGE_KEYS.explicitlySignedOut);
  const verifier = randomBase64Url(32);
  const state = randomBase64Url(16);
  const challenge = base64UrlEncode(await sha256(verifier));

  sessionStorage.setItem(STORAGE_KEYS.pkceVerifier, verifier);
  sessionStorage.setItem(STORAGE_KEYS.oauthState, state);
  if (currentPath) sessionStorage.setItem(STORAGE_KEYS.postLoginPath, currentPath);

  const params = new URLSearchParams({
    response_type: "code",
    client_id: CLIENT_ID,
    redirect_uri: redirectUri(),
    scope: SCOPE,
    state,
    code_challenge: challenge,
    code_challenge_method: "S256",
  });
  return `${oidcIssuer()}/oidc/authorize?${params}`;
}

export async function redirectToLogin(currentPath?: string): Promise<void> {
  window.location.assign(await createAuthorizationUrl(currentPath));
}

async function exchangeAuthorizationCode(code: string, state: string): Promise<string> {
  const expectedState = sessionStorage.getItem(STORAGE_KEYS.oauthState);
  if (!expectedState || expectedState !== state) {
    throw new Error("OAuth state mismatch; please sign in again");
  }

  const verifier = sessionStorage.getItem(STORAGE_KEYS.pkceVerifier);
  if (!verifier) throw new Error("Missing PKCE verifier; please sign in again");

  const body = new URLSearchParams({
    grant_type: "authorization_code",
    code,
    redirect_uri: redirectUri(),
    client_id: CLIENT_ID,
    code_verifier: verifier,
  });
  const response = await fetch(`${oidcIssuer()}/oidc/token`, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body,
  });
  if (!response.ok) {
    const detail = await response.text().catch(() => "");
    throw new Error(
      `Token exchange failed: ${response.status}${detail ? ` (${detail})` : ""}`,
    );
  }

  const tokens = (await response.json()) as OidcTokens;
  if (!tokens.access_token) throw new Error("Token exchange returned no access token");
  storeTokens(tokens);

  sessionStorage.removeItem(STORAGE_KEYS.pkceVerifier);
  sessionStorage.removeItem(STORAGE_KEYS.oauthState);
  const destination = sessionStorage.getItem(STORAGE_KEYS.postLoginPath) ?? "/chat";
  sessionStorage.removeItem(STORAGE_KEYS.postLoginPath);
  completedCallback = { code, state, destination };
  return destination;
}

export function handleCallback(code: string, state: string): Promise<string> {
  if (completedCallback?.code === code && completedCallback.state === state) {
    return Promise.resolve(completedCallback.destination);
  }
  if (callbackInFlight?.code === code && callbackInFlight.state === state) {
    return callbackInFlight.promise;
  }

  const promise = exchangeAuthorizationCode(code, state).finally(() => {
    callbackInFlight = null;
  });
  callbackInFlight = { code, state, promise };
  return promise;
}

export type RefreshTokenResult =
  // "ok": a fresh access token is available.
  | { status: "ok"; token: string }
  // "invalid": the stored credentials are conclusively rejected (or absent)
  // and local auth state has been cleared; the user must sign in again.
  | { status: "invalid" }
  // "error": a transient network or server failure; stored tokens are kept
  // so the session can recover once the identity provider is reachable.
  | { status: "error" };

async function refreshAccessTokenInner(): Promise<RefreshTokenResult> {
  const refreshToken = getRefreshToken();
  if (!refreshToken) return { status: "invalid" };

  try {
    const body = new URLSearchParams({
      grant_type: "refresh_token",
      refresh_token: refreshToken,
      client_id: CLIENT_ID,
    });
    const response = await fetch(`${oidcIssuer()}/oidc/token`, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body,
    });
    if (!response.ok) {
      // Only 400/401 conclusively mean the stored refresh token is invalid.
      // Keep tokens on transient server errors so the session can recover
      // after the identity provider comes back.
      if (response.status === 400 || response.status === 401) {
        clearAuth();
        return { status: "invalid" };
      }
      return { status: "error" };
    }
    const tokens = (await response.json()) as OidcTokens;
    if (!tokens.access_token) {
      clearAuth();
      return { status: "invalid" };
    }
    storeTokens(tokens);
    return { status: "ok", token: tokens.access_token };
  } catch {
    return { status: "error" };
  }
}

export function refreshAccessToken(): Promise<RefreshTokenResult> {
  if (!refreshInFlight) {
    refreshInFlight = refreshAccessTokenInner().finally(() => {
      refreshInFlight = null;
    });
  }
  return refreshInFlight;
}

export async function revokeAndClearAuth(): Promise<void> {
  const refreshToken = getRefreshToken();
  try {
    if (refreshToken) {
      await fetch(`${oidcIssuer()}/oidc/revoke`, {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({
          token: refreshToken,
          token_type_hint: "refresh_token",
          client_id: CLIENT_ID,
        }),
      });
    }
  } finally {
    clearAuth();
    localStorage.setItem(STORAGE_KEYS.explicitlySignedOut, "true");
  }
}

export function redirectToIdpLogout(): void {
  const params = new URLSearchParams({
    client_id: CLIENT_ID,
    post_logout_redirect_uri: `${window.location.origin}/login`,
  });
  window.location.replace(`${oidcIssuer()}/oidc/logout?${params}`);
}

export async function fetchUserInfo(): Promise<UserInfo | null> {
  const token = getAccessToken();
  if (!token) return null;
  try {
    const response = await fetch(`${oidcIssuer()}/api/v1/oidc/userinfo`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!response.ok) return null;
    return (await response.json()) as UserInfo;
  } catch {
    return null;
  }
}
