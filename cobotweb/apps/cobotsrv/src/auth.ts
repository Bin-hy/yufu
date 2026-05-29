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
const CLIENT_SECRET = import.meta.env.VITE_OIDC_CLIENT_SECRET ?? ''
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

/// Exchange the `code` returned to /oauth/callback for tokens. Returns the
/// path the user was originally trying to reach (or `/dashboard`).
export async function handleCallback(code: string, state: string): Promise<string> {
  const expected = sessionStorage.getItem(STORAGE_KEYS.oauthState)
  if (!expected || expected !== state) {
    throw new Error('OAuth state mismatch — possible CSRF attempt')
  }
  const verifier = sessionStorage.getItem(STORAGE_KEYS.pkceVerifier)
  if (!verifier) throw new Error('Missing PKCE verifier — re-login required')

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
  if (CLIENT_SECRET) body.set('client_secret', CLIENT_SECRET)

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
