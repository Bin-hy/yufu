import { randomUUID } from 'node:crypto'
import type { AddressInfo } from 'node:net'

import { generateKeyPair, SignJWT } from 'jose'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

import type { WebConfig } from '../src/config.js'
import {
  applyBackchannelLogout,
  BACKCHANNEL_LOGOUT_EVENT,
  buildEndSessionUrl,
  createLogoutTokenVerifier,
  LogoutReplayGuard,
  LogoutTokenError,
} from '../src/logout.js'
import type { OidcClient } from '../src/oidc.js'
import {
  authorizeOnlineOperation,
  createIntrospector,
  csrfToken,
  OperationDenied,
  originAllowed,
  verifyCsrfToken,
} from '../src/protection.js'
import { createApp } from '../src/server.js'
import { PendingStore, ProfileDirectory, SessionStore } from '../src/session.js'

const ISSUER = 'https://id.example.com'
const CLIENT_ID = 'cobotrix-web-example'

const config: WebConfig = {
  port: 3000,
  issuer: ISSUER,
  clientId: CLIENT_ID,
  clientSecret: 'web-client-secret',
  redirectUri: 'https://app.example.com/callback',
  postLogoutRedirectUri: 'https://app.example.com/',
  scope: 'openid profile email',
  sessionSecret: 's'.repeat(32),
  readerClientId: 'cobotrix-web-reader',
  readerClientSecret: 'reader-secret',
  allowInsecureLoopback: false,
}

const { publicKey, privateKey } = await generateKeyPair('RS256')
const other = await generateKeyPair('RS256')

function secondsFromNow(offset: number): number {
  return Math.floor(Date.now() / 1000) + offset
}

async function signLogout(
  overrides: Record<string, unknown> = {},
  key: CryptoKey = privateKey,
): Promise<string> {
  const payload = {
    iss: ISSUER,
    aud: CLIENT_ID,
    sub: 'user-1',
    sid: 'sid-1',
    iat: secondsFromNow(0),
    exp: secondsFromNow(120),
    jti: randomUUID(),
    events: { [BACKCHANNEL_LOGOUT_EVENT]: {} },
    ...overrides,
  }
  return new SignJWT(payload)
    .setProtectedHeader({ alg: 'RS256', typ: 'logout+jwt' })
    .sign(key)
}

const verify = createLogoutTokenVerifier({
  issuer: ISSUER,
  clientId: CLIENT_ID,
  key: publicKey,
})

function newSessionStore(): { sessions: SessionStore; profile: ReturnType<ProfileDirectory['upsert']> } {
  const sessions = new SessionStore(() => 1_000)
  const profile = new ProfileDirectory().upsert(
    { issuer: ISSUER, subject: 'user-1', displayName: 'Alice' },
    1_000,
  )
  return { sessions, profile }
}

describe('back-channel logout token', () => {
  it('accepts a well-formed notification', async () => {
    const verified = await verify(await signLogout())

    expect(verified.issuer).toBe(ISSUER)
    expect(verified.clientId).toBe(CLIENT_ID)
    expect(verified.sid).toBe('sid-1')
    expect(verified.subject).toBe('user-1')
  })

  it('rejects a token signed by another key', async () => {
    await expect(verify(await signLogout({}, other.privateKey))).rejects.toThrow(
      LogoutTokenError,
    )
  })

  it('rejects a token for another issuer or audience', async () => {
    await expect(verify(await signLogout({ iss: 'https://evil.example' }))).rejects.toThrow(
      LogoutTokenError,
    )
    await expect(verify(await signLogout({ aud: 'another-client' }))).rejects.toThrow(
      LogoutTokenError,
    )
  })

  it('rejects a token without the backchannel event', async () => {
    await expect(verify(await signLogout({ events: {} }))).rejects.toThrow(/backchannel event/)
  })

  it('rejects a token that carries nonce', async () => {
    await expect(verify(await signLogout({ nonce: 'n-1' }))).rejects.toThrow(/nonce/)
  })

  it('rejects a token without jti', async () => {
    await expect(verify(await signLogout({ jti: undefined }))).rejects.toThrow(/jti/)
  })

  it('rejects a token that identifies neither sid nor sub', async () => {
    await expect(
      verify(await signLogout({ sid: undefined, sub: undefined })),
    ).rejects.toThrow(/sid or sub/)
  })

  it('rejects an expired token', async () => {
    await expect(
      verify(await signLogout({ iat: secondsFromNow(-600), exp: secondsFromNow(-480) })),
    ).rejects.toThrow(LogoutTokenError)
  })
})

describe('back-channel logout application', () => {
  it('clears local sessions by sid and ignores repeats', async () => {
    const { sessions, profile } = newSessionStore()
    const tokens = { accessToken: 'a', expiresAt: 9_000 }
    const first = sessions.create(profile, tokens, 'sid-1')
    const second = sessions.create(profile, tokens, 'sid-2')
    const guard = new LogoutReplayGuard(60_000, () => 1_000)
    const token = await verify(await signLogout())

    expect(guard.seen(token.jti)).toBe(false)
    expect(applyBackchannelLogout(sessions, token)).toBe(1)
    expect(sessions.get(first.id)).toBeUndefined()
    expect(sessions.get(second.id)).toBeDefined()

    // A duplicate delivery is acknowledged but must not run side effects again.
    expect(guard.seen(token.jti)).toBe(true)
    expect(applyBackchannelLogout(sessions, token)).toBe(0)
  })

  it('falls back to sub when the notification has no sid', async () => {
    const { sessions, profile } = newSessionStore()
    const tokens = { accessToken: 'a', expiresAt: 9_000 }
    sessions.create(profile, tokens, 'sid-1')
    sessions.create(profile, tokens, 'sid-2')
    const token = await verify(await signLogout({ sid: undefined }))

    expect(applyBackchannelLogout(sessions, token)).toBe(2)
  })
})

describe('RP-initiated logout', () => {
  it('builds a standard end-session request', () => {
    const url = new URL(
      buildEndSessionUrl({
        endSessionEndpoint: `${ISSUER}/oidc/end-session`,
        idTokenHint: 'id-token',
        postLogoutRedirectUri: config.postLogoutRedirectUri,
        state: 'logout-state',
      }),
    )

    expect(url.origin + url.pathname).toBe(`${ISSUER}/oidc/end-session`)
    expect(url.searchParams.get('id_token_hint')).toBe('id-token')
    expect(url.searchParams.get('post_logout_redirect_uri')).toBe(
      config.postLogoutRedirectUri,
    )
    expect(url.searchParams.get('state')).toBe('logout-state')
  })
})

describe('local write protection', () => {
  it('binds the CSRF token to the local session', () => {
    const token = csrfToken('session-1', config.sessionSecret)

    expect(verifyCsrfToken('session-1', config.sessionSecret, token)).toBe(true)
    expect(verifyCsrfToken('session-2', config.sessionSecret, token)).toBe(false)
    expect(verifyCsrfToken('session-1', config.sessionSecret, undefined)).toBe(false)
  })

  it('accepts only the configured origin', () => {
    expect(originAllowed('https://app.example.com', config.redirectUri)).toBe(true)
    expect(originAllowed('https://evil.example', config.redirectUri)).toBe(false)
    expect(originAllowed(undefined, config.redirectUri)).toBe(false)
  })

  it('denies a write without a local session', async () => {
    await expect(
      authorizeOnlineOperation({
        session: undefined,
        introspector: { introspect: async () => ({ active: true }) },
        expectedClientId: CLIENT_ID,
        now: 1_000,
      }),
    ).rejects.toMatchObject({ reason: 'unauthenticated' })
  })

  it('denies inactive, foreign, mismatched and expired tokens', async () => {
    const { sessions, profile } = newSessionStore()
    const session = sessions.create(profile, { accessToken: 'a', expiresAt: 9_000 })
    const cases = [
      [{ active: false }, 'inactive_token'],
      [{ active: true, clientId: 'another-client', sub: 'user-1' }, 'wrong_client'],
      [{ active: true, clientId: CLIENT_ID, sub: 'user-2' }, 'wrong_subject'],
      [{ active: true, clientId: CLIENT_ID, sub: 'user-1', exp: 1 }, 'expired_token'],
    ] as const

    for (const [result, reason] of cases) {
      await expect(
        authorizeOnlineOperation({
          session,
          introspector: { introspect: async () => result },
          expectedClientId: CLIENT_ID,
          now: 1_000,
        }),
        reason,
      ).rejects.toMatchObject({ reason })
    }
  })

  it('fails closed when introspection is unavailable', async () => {
    const { sessions, profile } = newSessionStore()
    const session = sessions.create(profile, { accessToken: 'a', expiresAt: 9_000 })

    await expect(
      authorizeOnlineOperation({
        session,
        introspector: {
          introspect: async () => {
            throw new Error('network down')
          },
        },
        expectedClientId: CLIENT_ID,
        now: 1_000,
      }),
    ).rejects.toMatchObject({ reason: 'introspection_unavailable' })
  })

  it('allows an active token for this client and subject', async () => {
    const { sessions, profile } = newSessionStore()
    const session = sessions.create(profile, { accessToken: 'a', expiresAt: 9_000 })

    await expect(
      authorizeOnlineOperation({
        session,
        introspector: {
          introspect: async () => ({
            active: true,
            clientId: CLIENT_ID,
            sub: 'user-1',
            exp: 9_000,
          }),
        },
        expectedClientId: CLIENT_ID,
        now: 1_000,
      }),
    ).resolves.toMatchObject({ allowed: true, subject: 'user-1' })
  })

  it('sends reader credentials only as Basic auth to the introspection endpoint', async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const introspector = createIntrospector(config, {
      fetch: (async (url: string | URL | Request, init?: RequestInit) => {
        calls.push({ url: String(url), init: init ?? {} })
        return new Response(JSON.stringify({ active: true, client_id: CLIENT_ID }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }) as typeof fetch,
    })

    await introspector.introspect('access-token')

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe(`${ISSUER}/oidc/introspect`)
    const headers = calls[0]?.init.headers as Record<string, string>
    expect(headers.authorization).toBe(
      `Basic ${Buffer.from('cobotrix-web-reader:reader-secret').toString('base64')}`,
    )
    expect(String(calls[0]?.init.body)).not.toContain('reader-secret')
  })
})

describe('web example HTTP surface', () => {
  const sessions = new SessionStore()
  const profile = new ProfileDirectory().upsert(
    { issuer: ISSUER, subject: 'user-1', displayName: 'Alice' },
    1_000,
  )
  const session = sessions.create(profile, { accessToken: 'active', expiresAt: 9_000 }, 'sid-1')

  const oidc: OidcClient = {
    authorizationRequest: async () => {
      throw new Error('not used')
    },
    exchangeCode: async () => {
      throw new Error('not used')
    },
    refresh: async () => {
      throw new Error('not used')
    },
    revoke: async () => undefined,
    endSessionEndpoint: async () => `${ISSUER}/oidc/end-session`,
  }

  const app = createApp({
    config,
    oidc,
    sessions,
    profiles: new ProfileDirectory(),
    pending: new PendingStore(),
    introspector: {
      introspect: async (token) =>
        token === 'active'
          ? { active: true, clientId: CLIENT_ID, sub: 'user-1', exp: 9_000 }
          : { active: false },
    },
    verifyLogoutToken: verify,
    replayGuard: new LogoutReplayGuard(),
    now: () => 1_000,
  })

  const server = app.listen(0)
  let base = ''

  beforeAll(async () => {
    if (!server.listening) {
      await new Promise<void>((resolve) => server.once('listening', resolve))
    }
    base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`
  })

  afterAll(() => {
    server.close()
  })

  const post = (path: string, body: Record<string, string>, cookie?: string) =>
    fetch(`${base}${path}`, {
      method: 'POST',
      headers: {
        'content-type': 'application/x-www-form-urlencoded',
        origin: 'https://app.example.com',
        ...(cookie ? { cookie } : {}),
      },
      body: new URLSearchParams(body),
      redirect: 'manual',
    })

  it('rejects a forged back-channel notification', async () => {
    const response = await post('/oidc/backchannel-logout', {
      logout_token: await signLogout({}, other.privateKey),
    })

    expect(response.status).toBe(400)
  })

  it('accepts a valid notification and acknowledges a duplicate', async () => {
    const token = await signLogout({ sid: 'sid-1' })
    const first = await post('/oidc/backchannel-logout', { logout_token: token })
    const second = await post('/oidc/backchannel-logout', { logout_token: token })

    expect(first.status).toBe(200)
    expect(await first.json()).toEqual({ status: 'ok', removed: 1, cached: 0 })
    expect(second.status).toBe(200)
    expect(await second.json()).toEqual({ status: 'duplicate' })
  })

  it('rejects a protected write without a session', async () => {
    const response = await post('/api/operation', { _csrf: 'anything' })

    expect(response.status).toBe(401)
  })

  it('rejects a protected write with a bad CSRF token', async () => {
    const local = sessions.create(profile, { accessToken: 'active', expiresAt: 9_000 }, 'sid-csrf')
    const response = await post(
      '/api/operation',
      { _csrf: 'wrong' },
      `cobotrix_session=${local.id}`,
    )

    expect(response.status).toBe(403)
    expect(await response.json()).toEqual({ error: 'csrf_rejected' })
  })

  it('rejects a protected write when introspection is inactive', async () => {
    const local = sessions.create(profile, { accessToken: 'revoked', expiresAt: 9_000 })
    const response = await post(
      '/api/operation',
      { _csrf: csrfToken(local.id, config.sessionSecret) },
      `cobotrix_session=${local.id}`,
    )

    expect(response.status).toBe(403)
    expect(await response.json()).toEqual({ error: 'inactive_token' })
  })
})
