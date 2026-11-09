import { readFile, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import type { Express } from 'express'
import { exportJWK, generateKeyPair, SignJWT, type CryptoKey } from 'jose'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

import type { ApiConfig } from '../server/config.js'
import { createLogoutTokenVerifier, LogoutReplayGuard } from '../server/logout.js'
import {
  AccessTokenError,
  AuthorizationCache,
  createAccessTokenVerifier,
  createIntrospector,
  originAllowed,
  type IntrospectionResult,
  type Introspector,
} from '../server/protection.js'
import { createApi } from '../server/server.js'

const ISSUER = 'https://id.example.com'
const AUDIENCE = 'urn:app:production'
const CLIENT_ID = 'cobotrix-spa-example'
const OTHER_CLIENT_ID = 'cobotrix-other-app'

const config: ApiConfig = {
  port: 0,
  issuer: ISSUER,
  audience: AUDIENCE,
  clientId: CLIENT_ID,
  readerClientId: 'cobotrix-spa-reader',
  readerClientSecret: 'reader-secret',
  allowedOrigin: 'https://spa.example.com',
  allowInsecureLoopback: false,
}

let privateKey: CryptoKey
let publicKey: CryptoKey
let foreignPrivateKey: CryptoKey

async function accessToken(
  overrides: {
    issuer?: string
    audience?: string
    subject?: string
    expiresAt?: number
    sessionId?: string
    typ?: string
    key?: CryptoKey
  } = {},
): Promise<string> {
  const now = Math.floor(Date.now() / 1000)
  return new SignJWT({
    session_id: overrides.sessionId ?? 'credential-session-1',
    application: CLIENT_ID,
    scope: 'openid profile',
  })
    .setProtectedHeader({ alg: 'RS256', typ: overrides.typ ?? 'at+jwt' })
    .setIssuer(overrides.issuer ?? ISSUER)
    .setAudience(overrides.audience ?? AUDIENCE)
    .setSubject(overrides.subject ?? 'user-1')
    .setIssuedAt(now)
    .setExpirationTime(overrides.expiresAt ?? now + 300)
    .setJti('access-jti-1')
    .sign(overrides.key ?? privateKey)
}

async function logoutToken(
  overrides: {
    audience?: string
    subject?: string
    sid?: string
    jti?: string
    nonce?: string
    events?: Record<string, unknown>
    key?: CryptoKey
    expiresAt?: number
  } = {},
): Promise<string> {
  const now = Math.floor(Date.now() / 1000)
  const payload: Record<string, unknown> = {
    events:
      overrides.events ??
      { 'http://schemas.openid.net/event/backchannel-logout': {} },
    sid: overrides.sid ?? 'protocol-session-1',
  }
  if (overrides.subject) payload.sub = overrides.subject
  if (overrides.nonce) payload.nonce = overrides.nonce
  return new SignJWT(payload)
    .setProtectedHeader({ alg: 'RS256', typ: 'logout+jwt' })
    .setIssuer(ISSUER)
    .setAudience(overrides.audience ?? CLIENT_ID)
    .setIssuedAt(now)
    .setJti(overrides.jti ?? 'logout-jti-1')
    .setExpirationTime(overrides.expiresAt ?? now + 120)
    .sign(overrides.key ?? privateKey)
}

interface RunningApi {
  url: string
  close: () => Promise<void>
}

async function start(app: Express): Promise<RunningApi> {
  const server = app.listen(0)
  await new Promise<void>((resolve) => server.once('listening', () => resolve()))
  const address = server.address()
  const port =
    typeof address === 'object' && address !== null ? address.port : 0
  return {
    url: `http://127.0.0.1:${port}`,
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      ),
  }
}

function introspectorReturning(result: IntrospectionResult): Introspector {
  return { introspect: async () => result }
}

async function buildApi(
  introspector: Introspector,
  cache = new AuthorizationCache(),
): Promise<RunningApi> {
  const app = createApi({
    config,
    verifyAccessToken: createAccessTokenVerifier({
      issuer: ISSUER,
      audience: AUDIENCE,
      key: publicKey,
    }),
    introspector,
    cache,
    verifyLogoutToken: createLogoutTokenVerifier({
      issuer: ISSUER,
      clientId: CLIENT_ID,
      key: publicKey,
    }),
    replayGuard: new LogoutReplayGuard(),
  })
  return start(app)
}

const active: IntrospectionResult = {
  active: true,
  sub: 'user-1',
  clientId: CLIENT_ID,
  scope: 'openid profile',
  exp: Math.floor(Date.now() / 1000) + 300,
  sid: 'protocol-session-1',
}

beforeAll(async () => {
  const pair = await generateKeyPair('RS256')
  privateKey = pair.privateKey
  publicKey = pair.publicKey
  foreignPrivateKey = (await generateKeyPair('RS256')).privateKey
  // Fail fast if the JWK export path used by the issuer is unavailable.
  expect((await exportJWK(publicKey)).kty).toBe('RSA')
})

describe('protected API bearer verification', () => {
  let api: RunningApi
  let calls = 0

  beforeAll(async () => {
    calls = 0
    api = await buildApi({
      introspect: async () => {
        calls += 1
        return active
      },
    })
  })

  afterAll(async () => {
    await api.close()
  })

  async function call(token?: string, origin?: string): Promise<Response> {
    const headers: Record<string, string> = {}
    if (token) headers.authorization = `Bearer ${token}`
    if (origin) headers.origin = origin
    return fetch(`${api.url}/api/operation`, { method: 'POST', headers })
  }

  it('accepts a signed token for this issuer, audience and client', async () => {
    const response = await call(await accessToken())

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      ok: true,
      operation: 'example-write',
      subject: 'user-1',
    })
    expect(calls).toBe(1)
  })

  it('rejects a missing bearer token without introspecting', async () => {
    const before = calls
    const response = await call()

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'missing_bearer_token' })
    expect(calls).toBe(before)
  })

  it('rejects a token signed by another key', async () => {
    const response = await call(await accessToken({ key: foreignPrivateKey }))

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'invalid_token' })
  })

  it('rejects a token from another issuer', async () => {
    const response = await call(await accessToken({ issuer: 'https://evil.example.com' }))

    expect(response.status).toBe(401)
  })

  it('rejects a token minted for another application audience', async () => {
    const response = await call(await accessToken({ audience: 'urn:app:other' }))

    expect(response.status).toBe(401)
  })

  it('rejects an expired token', async () => {
    const response = await call(
      await accessToken({ expiresAt: Math.floor(Date.now() / 1000) - 3600 }),
    )

    expect(response.status).toBe(401)
  })

  it('rejects an ID token replayed as an access token', async () => {
    const response = await call(await accessToken({ typ: 'JWT' }))

    expect(response.status).toBe(401)
  })
})

describe('protected API online admission', () => {
  async function callWith(
    introspector: Introspector,
    token: string,
  ): Promise<Response> {
    const api = await buildApi(introspector)
    try {
      return await fetch(`${api.url}/api/operation`, {
        method: 'POST',
        headers: { authorization: `Bearer ${token}` },
      })
    } finally {
      await api.close()
    }
  }

  it('rejects an inactive token', async () => {
    const response = await callWith(introspectorReturning({ active: false }), await accessToken())

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'inactive_token' })
  })

  it('rejects a token minted for another client', async () => {
    const response = await callWith(
      introspectorReturning({ ...active, clientId: OTHER_CLIENT_ID }),
      await accessToken(),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'wrong_client' })
  })

  it('rejects a token whose subject does not match the JWT', async () => {
    const response = await callWith(
      introspectorReturning({ ...active, sub: 'user-2' }),
      await accessToken(),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'wrong_subject' })
  })

  it('rejects a token the provider reports as expired', async () => {
    const response = await callWith(
      introspectorReturning({
        ...active,
        exp: Math.floor(Date.now() / 1000) - 1,
      }),
      await accessToken(),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'expired_token' })
  })

  it('fails closed when introspection is unavailable', async () => {
    const response = await callWith(
      {
        introspect: async () => {
          throw new Error('connection refused')
        },
      },
      await accessToken(),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({
      error: 'introspection_unavailable',
    })
  })

  it('rejects a token that is not a verifiable access token', async () => {
    const response = await callWith(introspectorReturning(active), 'not-a-jwt')

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'invalid_token' })
  })
})

describe('protected API origin gate', () => {
  let api: RunningApi

  beforeAll(async () => {
    api = await buildApi(introspectorReturning(active))
  })

  afterAll(async () => {
    await api.close()
  })

  it('allows the registered SPA origin and echoes CORS headers', async () => {
    const response = await fetch(`${api.url}/api/operation`, {
      method: 'POST',
      headers: {
        authorization: `Bearer ${await accessToken()}`,
        origin: 'https://spa.example.com',
      },
    })

    expect(response.status).toBe(200)
    expect(response.headers.get('access-control-allow-origin')).toBe(
      'https://spa.example.com',
    )
    expect(response.headers.get('vary')).toBe('Origin')
  })

  it('rejects an unregistered origin before token handling', async () => {
    const response = await fetch(`${api.url}/api/operation`, {
      method: 'POST',
      headers: {
        authorization: `Bearer ${await accessToken()}`,
        origin: 'https://evil.example.com',
      },
    })

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'origin_rejected' })
  })

  it('answers preflight from the registered origin', async () => {
    const response = await fetch(`${api.url}/api/operation`, {
      method: 'OPTIONS',
      headers: { origin: 'https://spa.example.com' },
    })

    expect(response.status).toBe(204)
    expect(response.headers.get('access-control-allow-methods')).toBe('POST, OPTIONS')
  })

  it('matches origins exactly, including scheme and port', () => {
    expect(originAllowed('https://spa.example.com', 'https://spa.example.com')).toBe(true)
    expect(originAllowed('https://spa.example.com/', 'https://spa.example.com')).toBe(true)
    expect(originAllowed('http://spa.example.com', 'https://spa.example.com')).toBe(false)
    expect(originAllowed('https://spa.example.com:8443', 'https://spa.example.com')).toBe(false)
    expect(originAllowed('https://spa.example.com.evil.test', 'https://spa.example.com')).toBe(
      false,
    )
    expect(originAllowed(undefined, 'https://spa.example.com')).toBe(false)
    expect(originAllowed('not a url', 'https://spa.example.com')).toBe(false)
  })
})

describe('protected API back-channel logout', () => {
  it('clears cached authorizations named by the notification', async () => {
    const cache = new AuthorizationCache()
    const token = await accessToken()
    const api = await buildApi(introspectorReturning(active), cache)
    try {
      const first = await fetch(`${api.url}/api/operation`, {
        method: 'POST',
        headers: { authorization: `Bearer ${token}` },
      })
      expect(first.status).toBe(200)
      expect(cache.size).toBe(1)

      const logout = await fetch(`${api.url}/oidc/backchannel-logout`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ logout_token: await logoutToken() }),
      })
      expect(logout.status).toBe(200)
      await expect(logout.json()).resolves.toEqual({ status: 'ok', removed: 1 })
      expect(cache.size).toBe(0)
    } finally {
      await api.close()
    }
  })

  it('is idempotent for a repeated jti', async () => {
    const cache = new AuthorizationCache()
    const api = await buildApi(introspectorReturning(active), cache)
    try {
      const token = await logoutToken({ jti: 'repeat-jti' })
      const send = () =>
        fetch(`${api.url}/oidc/backchannel-logout`, {
          method: 'POST',
          headers: { 'content-type': 'application/x-www-form-urlencoded' },
          body: new URLSearchParams({ logout_token: token }),
        })

      expect((await send()).status).toBe(200)
      const duplicate = await send()
      expect(duplicate.status).toBe(200)
      await expect(duplicate.json()).resolves.toEqual({ status: 'duplicate' })
    } finally {
      await api.close()
    }
  })

  it('rejects a logout token for another client', async () => {
    const api = await buildApi(introspectorReturning(active))
    try {
      const response = await fetch(`${api.url}/oidc/backchannel-logout`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
          logout_token: await logoutToken({ audience: OTHER_CLIENT_ID }),
        }),
      })

      expect(response.status).toBe(400)
      await expect(response.json()).resolves.toEqual({ error: 'invalid_logout_token' })
    } finally {
      await api.close()
    }
  })

  it('rejects a logout token that carries a nonce', async () => {
    const api = await buildApi(introspectorReturning(active))
    try {
      const response = await fetch(`${api.url}/oidc/backchannel-logout`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
          logout_token: await logoutToken({ nonce: 'n-1' }),
        }),
      })

      expect(response.status).toBe(400)
    } finally {
      await api.close()
    }
  })

  it('rejects a logout token without the backchannel event', async () => {
    const api = await buildApi(introspectorReturning(active))
    try {
      const response = await fetch(`${api.url}/oidc/backchannel-logout`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
          logout_token: await logoutToken({ events: { 'urn:other': {} } }),
        }),
      })

      expect(response.status).toBe(400)
    } finally {
      await api.close()
    }
  })

  it('rejects a logout token signed by another key', async () => {
    const api = await buildApi(introspectorReturning(active))
    try {
      const response = await fetch(`${api.url}/oidc/backchannel-logout`, {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
          logout_token: await logoutToken({ key: foreignPrivateKey }),
        }),
      })

      expect(response.status).toBe(400)
    } finally {
      await api.close()
    }
  })
})

describe('reader-authenticated introspection', () => {
  it('sends the reader credentials only from the API process', async () => {
    let captured: { url: string; init: RequestInit } | undefined
    const introspector = createIntrospector(config, {
      endpoint: 'https://id.example.com/oidc/introspect',
      fetch: (async (url: string | URL | Request, init?: RequestInit) => {
        captured = { url: String(url), init: init ?? {} }
        return new Response(
          JSON.stringify({
            active: true,
            sub: 'user-1',
            client_id: CLIENT_ID,
            scope: 'openid',
            exp: 1_900_000_000,
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        )
      }) as typeof fetch,
    })

    const result = await introspector.introspect('the-access-token')

    expect(result).toEqual({
      active: true,
      sub: 'user-1',
      clientId: CLIENT_ID,
      scope: 'openid',
      exp: 1_900_000_000,
      sid: undefined,
    })
    expect(captured?.url).toBe('https://id.example.com/oidc/introspect')
    const headers = new Headers(captured?.init.headers)
    expect(headers.get('authorization')).toBe(
      `Basic ${Buffer.from('cobotrix-spa-reader:reader-secret').toString('base64')}`,
    )
    expect(String(captured?.init.body)).toBe(
      'token=the-access-token&token_type_hint=access_token',
    )
  })

  it('fails closed when the provider rejects the reader credentials', async () => {
    const introspector = createIntrospector(config, {
      fetch: (async () => new Response('', { status: 401 })) as typeof fetch,
    })

    await expect(introspector.introspect('token')).rejects.toThrow(/status 401/)
  })

  it('never exposes reader credentials to browser sources', async () => {
    const root = fileURLToPath(new URL('../src', import.meta.url))
    const entries = await readdir(root, { recursive: true, withFileTypes: true })
    const files = entries
      .filter((entry) => entry.isFile() && entry.name.endsWith('.ts'))
      .map((entry) => join(entry.parentPath, entry.name))

    expect(files.length).toBeGreaterThan(0)
    for (const file of files) {
      const source = await readFile(file, 'utf8')
      // Naming a forbidden variable in a guard is fine; reading it is not.
      expect(source, file).not.toMatch(/readerClientSecret/)
      expect(source, file).not.toMatch(
        /env\.READER_CLIENT_SECRET|env\['READER_CLIENT_SECRET'\]/,
      )
      expect(source, file).not.toMatch(/from '\\.\\.\/server\//)
    }
  })
})

describe('access token verifier', () => {
  it('reports a verification failure as AccessTokenError', async () => {
    const verify = createAccessTokenVerifier({
      issuer: ISSUER,
      audience: AUDIENCE,
      key: publicKey,
    })

    await expect(verify('broken')).rejects.toBeInstanceOf(AccessTokenError)
  })
})
