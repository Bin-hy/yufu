/// OIDC Authorization Code + PKCE flow for cobotsrv.
///
/// cobotsrv is a public OIDC client (no client_secret). PKCE provides
/// proof-of-possession — cobotsrv accepts authorization_code tokens without
/// client_secret when PKCE is used (RFC 7636).

const STORAGE_KEYS = {
  accessToken: 'cobotsrv.access_token',
  refreshToken: 'cobotsrv.refresh_token',
  expiresAt: 'cobotsrv.expires_at',
  pkceVerifier: 'cobotsrv.pkce_verifier',
  oauthState: 'cobotsrv.oauth_state',
  postLoginPath: 'cobotsrv.post_login_path',
} as const

const COBOTSRV_BASE = (import.meta.env.VITE_COBOTSRV_URL ?? '').replace(/\/$/, '')
const CLIENT_ID = import.meta.env.VITE_OIDC_CLIENT_ID ?? 'cobotsrv'
const REDIRECT_URI =
  import.meta.env.VITE_OIDC_REDIRECT_URI ?? `${window.location.origin}/oauth/callback`
const SCOPE = 'openid profile email'

// ── token storage ──

export function getAccessToken(): string | null {
  const tok = localStorage.getItem(STORAGE_KEYS.accessToken)
  if (!tok) return null
  const exp = Number(localStorage.getItem(STORAGE_KEYS.expiresAt) ?? 0)
  if (exp && Date.now() > exp - 30_000 /* 30s clock skew buffer */) return null
  return tok
}

export function getRefreshToken(): string | null {
  return localStorage.getItem(STORAGE_KEYS.refreshToken)
}

export function isAuthenticated(): boolean {
  return getAccessToken() !== null || getRefreshToken() !== null
}

function storeTokens(payload: {
  access_token: string
  refresh_token?: string
  expires_in?: number
}) {
  localStorage.setItem(STORAGE_KEYS.accessToken, payload.access_token)
  if (payload.refresh_token) {
    localStorage.setItem(STORAGE_KEYS.refreshToken, payload.refresh_token)
  }
  const ttlMs = (payload.expires_in ?? 3600) * 1000
  localStorage.setItem(STORAGE_KEYS.expiresAt, String(Date.now() + ttlMs))
}

export function clearAuth(): void {
  localStorage.removeItem(STORAGE_KEYS.accessToken)
  localStorage.removeItem(STORAGE_KEYS.refreshToken)
  localStorage.removeItem(STORAGE_KEYS.expiresAt)
}

// ── PKCE primitives ──

function randomBase64Url(byteLength: number): string {
  const buf = new Uint8Array(byteLength)
  crypto.getRandomValues(buf)
  return base64UrlEncode(buf)
}

function base64UrlEncode(input: ArrayBuffer | Uint8Array): string {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input)
  let str = ''
  for (let i = 0; i < bytes.length; i++) str += String.fromCharCode(bytes[i])
  return btoa(str).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

async function sha256(input: string): Promise<ArrayBuffer> {
  return crypto.subtle.digest('SHA-256', new TextEncoder().encode(input))
}

// ── flow ──

export async function redirectToLogin(currentPath?: string): Promise<void> {
  const verifier = randomBase64Url(32)
  const state = randomBase64Url(16)
  const challenge = base64UrlEncode(await sha256(verifier))

  sessionStorage.setItem(STORAGE_KEYS.pkceVerifier, verifier)
  sessionStorage.setItem(STORAGE_KEYS.oauthState, state)
  if (currentPath) sessionStorage.setItem(STORAGE_KEYS.postLoginPath, currentPath)

  const params = new URLSearchParams({
    response_type: 'code',
    client_id: CLIENT_ID,
    redirect_uri: REDIRECT_URI,
    scope: SCOPE,
    state,
    code_challenge: challenge,
    code_challenge_method: 'S256',
  })
  window.location.assign(`${COBOTSRV_BASE}/oidc/authorize?${params}`)
}

/**
 * Redirect to the IdP login form directly (bypasses /oidc/authorize so the
 * user always sees the username/password screen even when an IdP session cookie
 * is still active). Used by the logout page's "重新登录" button.
 */
export async function redirectToLoginForm(currentPath?: string): Promise<void> {
  const verifier = randomBase64Url(32)
  const state = randomBase64Url(16)
  const challenge = base64UrlEncode(await sha256(verifier))

  sessionStorage.setItem(STORAGE_KEYS.pkceVerifier, verifier)
  sessionStorage.setItem(STORAGE_KEYS.oauthState, state)
  if (currentPath) sessionStorage.setItem(STORAGE_KEYS.postLoginPath, currentPath)

  const params = new URLSearchParams({
    response_type: 'code',
    client_id: CLIENT_ID,
    redirect_uri: REDIRECT_URI,
    scope: SCOPE,
    state,
    code_challenge: challenge,
    code_challenge_method: 'S256',
  })
  window.location.assign(`${COBOTSRV_BASE}/oidc/login?${params}`)
}

/// Exchange the `code` returned to /oauth/callback for tokens. Returns the
/// path the user was originally trying to reach (or `/dashboard`).
export async function handleCallback(code: string, state: string): Promise<string> {
  const expected = sessionStorage.getItem(STORAGE_KEYS.oauthState)
  if (!expected || expected !== state) {
    throw new Error('OAuth state mismatch — possible CSRF attempt')
  }
  const verifier = sessionStorage.getItem(STORAGE_KEYS.pkceVerifier)
  if (!verifier) throw new Error('Missing PKCE verifier — re-login required')

  // cobotsrv is a public OIDC client: PKCE proves possession, so no
  // client_secret is sent — embedding a secret in the SPA bundle would
  // expose it to anyone loading the page.
  const body = new URLSearchParams({
    grant_type: 'authorization_code',
    code,
    redirect_uri: REDIRECT_URI,
    client_id: CLIENT_ID,
    code_verifier: verifier,
  })

  const resp = await fetch(`${COBOTSRV_BASE}/oidc/token`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body,
  })
  if (!resp.ok) {
    throw new Error(`Token exchange failed: ${resp.status} ${await resp.text()}`)
  }
  const tokens = await resp.json() as {
    access_token: string
    refresh_token?: string
    expires_in?: number
  }
  storeTokens(tokens)

  // cleanup transient state
  sessionStorage.removeItem(STORAGE_KEYS.pkceVerifier)
  sessionStorage.removeItem(STORAGE_KEYS.oauthState)
  const dest = sessionStorage.getItem(STORAGE_KEYS.postLoginPath) ?? '/dashboard'
  sessionStorage.removeItem(STORAGE_KEYS.postLoginPath)
  return dest
}

/// Exchange the stored refresh_token for a new access_token. Returns the
/// new access token, or null if refresh failed (caller should re-login).
export async function refreshAccessToken(): Promise<string | null> {
  const rt = getRefreshToken()
  if (!rt) return null

  const body = new URLSearchParams({
    grant_type: 'refresh_token',
    refresh_token: rt,
    client_id: CLIENT_ID,
  })

  try {
    const resp = await fetch(`${COBOTSRV_BASE}/oidc/token`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body,
    })
    if (!resp.ok) {
      clearAuth()
      return null
    }
    const tokens = await resp.json() as {
      access_token: string
      refresh_token?: string
      expires_in?: number
    }
    storeTokens(tokens)
    return tokens.access_token
  } catch {
    clearAuth()
    return null
  }
}

/** Revoke a token server-side via RFC 7009. Failures are silent — we always clear locally. */
async function revokeToken(token: string, hint: 'refresh_token' | 'access_token'): Promise<void> {
  try {
    await fetch(`${COBOTSRV_BASE}/oidc/revoke`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ token, token_type_hint: hint }),
    })
  } catch {
    // network error — still clear locally below
  }
}

export async function logout(): Promise<void> {
  const rt = getRefreshToken()
  if (rt) await revokeToken(rt, 'refresh_token')
  clearAuth()
  window.location.href = '/logout'
}

// ── UserInfo (OIDC /userinfo endpoint) ──

export interface UserInfo {
  sub: number
  preferred_username?: string
  name?: string
  nickname?: string
  email?: string
}

/** Fetch user profile from cobotsrv's OIDC /userinfo endpoint. */
export async function fetchUserInfo(): Promise<UserInfo | null> {
  const tok = getAccessToken()
  if (!tok) return null
  try {
    const resp = await fetch(`${COBOTSRV_BASE}/api/v1/oidc/userinfo`, {
      headers: { Authorization: `Bearer ${tok}` },
    })
    if (!resp.ok) return null
    return (await resp.json()) as UserInfo
  } catch {
    return null
  }
}

// ── MCP access tokens (long-lived, persisted + revocable) ──
//
// These hit cobotusr (the IdP) at `COBOTSRV_BASE`, authenticated with the current session
// token. Each issued token is persisted server-side; the full token value is only returned
// once (on create/refresh) and never listed afterward.

/** Returned once on create/refresh — carries the actual token value. */
export interface McpTokenResponse {
  id: number
  name: string
  access_token: string
  token_type: string
  expires_in: number
  expires_at: string
  aud: string
}

/** Listed metadata — never includes the token value. */
export interface McpTokenInfo {
  id: number
  name: string
  aud: string
  created_at: string
  expires_at: string
}

/** Authenticated fetch against the IdP; throws with a readable message on non-2xx. */
async function idpFetch(path: string, init: RequestInit, action: string): Promise<Response> {
  const doFetch = (tok: string) =>
    fetch(`${COBOTSRV_BASE}${path}`, {
      ...init,
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${tok}`,
        ...(init.headers ?? {}),
      },
    })

  // Access token missing or within the expiry buffer — try one refresh first.
  let tok = getAccessToken()
  if (!tok) tok = await refreshAccessToken()
  if (!tok) throw new Error('未登录或会话已过期，请重新登录')

  let resp = await doFetch(tok)
  if (resp.status === 401) {
    // Single refresh-and-retry on 401 (e.g. token rejected early server-side).
    const newTok = await refreshAccessToken()
    if (newTok) resp = await doFetch(newTok)
  }
  if (!resp.ok) {
    throw new Error(`${action}失败: ${resp.status} ${await resp.text()}`)
  }
  return resp
}

/**
 * Create a long-lived MCP access token. The IdP signs a token scoped to `aud` carrying the
 * caller's resolved permissions (must include mcp:access) and persists a revocable record.
 */
export async function issueMcpToken(
  aud: string,
  ttlDays: number,
  name?: string,
): Promise<McpTokenResponse> {
  const resp = await idpFetch(
    '/api/v1/oidc/mcp-tokens',
    { method: 'POST', body: JSON.stringify({ aud, ttl_days: ttlDays, name }) },
    '生成令牌',
  )
  return (await resp.json()) as McpTokenResponse
}

/** List the caller's active (non-revoked) MCP tokens (metadata only). */
export async function listMcpTokens(): Promise<McpTokenInfo[]> {
  const resp = await idpFetch('/api/v1/oidc/mcp-tokens', { method: 'GET' }, '获取令牌列表')
  return (await resp.json()) as McpTokenInfo[]
}

/** Rename one of the caller's tokens. */
export async function renameMcpToken(id: number, name: string): Promise<void> {
  await idpFetch(
    `/api/v1/oidc/mcp-tokens/${id}`,
    { method: 'PATCH', body: JSON.stringify({ name }) },
    '重命名令牌',
  )
}

/** Rotate one of the caller's tokens: mints a new value, revokes the old one. */
export async function refreshMcpToken(id: number, ttlDays: number): Promise<McpTokenResponse> {
  const resp = await idpFetch(
    `/api/v1/oidc/mcp-tokens/${id}/refresh`,
    { method: 'POST', body: JSON.stringify({ ttl_days: ttlDays }) },
    '刷新令牌',
  )
  return (await resp.json()) as McpTokenResponse
}

/** Revoke (delete) one of the caller's tokens. */
export async function deleteMcpToken(id: number): Promise<void> {
  await idpFetch(`/api/v1/oidc/mcp-tokens/${id}`, { method: 'DELETE' }, '删除令牌')
}
