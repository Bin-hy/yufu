import * as client from 'openid-client'
import { describe, expect, it, vi } from 'vitest'

import type { WebConfig } from '../src/config.js'
import {
  assertAuthorizationResponse,
  createOidcClient,
  OidcError,
  parseAuthorizationOptions,
  pkceChallenge,
  tokenFingerprint,
  toIdentityClaims,
  toProtocolEvidence,
} from '../src/oidc.js'
import {
  PendingStore,
  ProfileDirectory,
  pendingCookie,
  safeReturnTo,
  sessionCookie,
  SessionStore,
} from '../src/session.js'

const config: WebConfig = {
  port: 3000,
  issuer: 'https://id.example.com',
  clientId: 'cobotrix-web-example',
  clientSecret: 'web-client-secret',
  redirectUri: 'https://app.example.com/callback',
  postLogoutRedirectUri: 'https://app.example.com/',
  scope: 'openid profile email',
  sessionSecret: 'x'.repeat(32),
  readerClientId: 'cobotrix-web-reader',
  readerClientSecret: 'reader-secret',
  allowInsecureLoopback: false,
}

function offlineClient(): ReturnType<typeof createOidcClient> {
  const configuration = new client.Configuration(
    {
      issuer: config.issuer,
      authorization_endpoint: `${config.issuer}/oidc/authorize`,
      token_endpoint: `${config.issuer}/oidc/token`,
      jwks_uri: `${config.issuer}/.well-known/jwks.json`,
      response_types_supported: ['code'],
      id_token_signing_alg_values_supported: ['RS256'],
      code_challenge_methods_supported: ['S256'],
    },
    config.clientId,
    { client_secret: config.clientSecret },
  )
  return createOidcClient(config, { configuration: async () => configuration })
}

describe('authorization request', () => {
  it('derives the RFC 7636 S256 challenge', () => {
    expect(pkceChallenge('dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk')).toBe(
      'E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM',
    )
  })

  it('sends state, nonce and a matching S256 challenge', async () => {
    const request = await offlineClient().authorizationRequest()
    const url = new URL(request.url)

    expect(url.origin + url.pathname).toBe('https://id.example.com/oidc/authorize')
    expect(url.searchParams.get('client_id')).toBe(config.clientId)
    expect(url.searchParams.get('redirect_uri')).toBe(config.redirectUri)
    expect(url.searchParams.get('response_type')).toBe('code')
    expect(url.searchParams.get('scope')).toBe('openid profile email')
    expect(url.searchParams.get('state')).toBe(request.state)
    expect(url.searchParams.get('nonce')).toBe(request.nonce)
    expect(url.searchParams.get('code_challenge_method')).toBe('S256')
    expect(url.searchParams.get('code_challenge')).toBe(pkceChallenge(request.codeVerifier))
    expect(url.searchParams.has('client_secret')).toBe(false)
  })

  it('forwards only whitelisted prompt and max_age values', async () => {
    const request = await offlineClient().authorizationRequest({
      prompt: 'login consent',
      maxAge: 0,
    })
    const url = new URL(request.url)

    expect(url.searchParams.get('prompt')).toBe('login consent')
    expect(url.searchParams.get('max_age')).toBe('0')
    expect(url.searchParams.get('nonce')).toBe(request.nonce)
    expect(url.searchParams.get('code_challenge_method')).toBe('S256')
  })

  it('rejects unsupported prompt and max_age values', () => {
    expect(() => parseAuthorizationOptions('none login', undefined)).toThrow(OidcError)
    expect(() => parseAuthorizationOptions('login', '-1')).toThrow(OidcError)
    expect(() => parseAuthorizationOptions('login', '4294967296')).toThrow(OidcError)
    expect(parseAuthorizationOptions(undefined, undefined)).toEqual({})
    expect(parseAuthorizationOptions('none', '0')).toEqual({ prompt: 'none', maxAge: 0 })
  })

  it('generates a fresh state and nonce for every attempt', async () => {
    const oidc = offlineClient()
    const first = await oidc.authorizationRequest()
    const second = await oidc.authorizationRequest()

    expect(first.state).not.toBe(second.state)
    expect(first.nonce).not.toBe(second.nonce)
    expect(first.codeVerifier).not.toBe(second.codeVerifier)
  })
})

describe('authorization response', () => {
  it('rejects an error callback without exchanging a code', () => {
    const url = new URL(
      'https://app.example.com/callback?error=access_denied&error_description=user+cancelled',
    )

    expect(() => assertAuthorizationResponse(url)).toThrow(OidcError)
    expect(() => assertAuthorizationResponse(url)).toThrow(/access_denied/)
  })

  it('rejects a callback without a code', () => {
    expect(() =>
      assertAuthorizationResponse(new URL('https://app.example.com/callback?state=abc')),
    ).toThrow(/missing code/)
  })

  it('returns the code for a valid callback', () => {
    expect(
      assertAuthorizationResponse(
        new URL('https://app.example.com/callback?code=abc&state=xyz'),
      ),
    ).toBe('abc')
  })
})

describe('pending authorization state', () => {
  it('is single use', () => {
    const store = new PendingStore(60_000, () => 1_000)
    store.put({ state: 's1', nonce: 'n1', codeVerifier: 'v1', returnTo: '/' })

    expect(store.consume('s1')?.nonce).toBe('n1')
    expect(store.consume('s1')).toBeUndefined()
  })

  it('expires after the authorization window', () => {
    let now = 1_000
    const store = new PendingStore(60_000, () => now)
    store.put({ state: 's1', nonce: 'n1', codeVerifier: 'v1', returnTo: '/' })

    now += 60_001
    expect(store.consume('s1')).toBeUndefined()
  })

  it('only accepts same-origin return paths', () => {
    expect(safeReturnTo('/settings?tab=profile')).toBe('/settings?tab=profile')
    expect(safeReturnTo('https://evil.example/')).toBe('/')
    expect(safeReturnTo('//evil.example/')).toBe('/')
    expect(safeReturnTo('/\\evil.example')).toBe('/')
    expect(safeReturnTo(undefined)).toBe('/')
  })
})

describe('local profile and session', () => {
  it('keys the local profile by (iss, sub)', () => {
    const profiles = new ProfileDirectory()
    const first = profiles.upsert(
      {
        issuer: 'https://a.example',
        subject: 'user-1',
        displayName: 'Alice',
        audience: [config.clientId],
      },
      1_000,
    )
    profiles.upsert(
      {
        issuer: 'https://b.example',
        subject: 'user-1',
        displayName: 'Alice B',
        audience: [config.clientId],
      },
      1_100,
    )
    const again = profiles.upsert(
      {
        issuer: 'https://a.example',
        subject: 'user-1',
        displayName: 'Alice A',
        audience: [config.clientId],
      },
      1_200,
    )

    expect(profiles.size).toBe(2)
    expect(again.createdAt).toBe(first.createdAt)
    expect(again.lastLoginAt).toBe(1_200)
    expect(again.displayName).toBe('Alice A')
  })

  it('keeps validated protocol evidence non-secret', () => {
    const claims = toIdentityClaims({
      iss: config.issuer,
      sub: 'user-1',
      aud: [config.clientId, 'other-client'],
      iat: 100,
      exp: 200,
      nonce: 'request-nonce',
      auth_time: 90,
      sid: 'sid-1',
      preferred_username: 'alice',
    } as Parameters<typeof toIdentityClaims>[0])
    const evidence = toProtocolEvidence(claims, config.clientId)

    expect(evidence).toEqual({
      issuer: config.issuer,
      subject: 'user-1',
      clientId: config.clientId,
      audience: [config.clientId, 'other-client'],
      sid: 'sid-1',
      authTime: 90,
      stateVerified: true,
      nonceVerified: true,
    })
    expect(JSON.stringify(evidence)).not.toContain('request-nonce')
    expect(tokenFingerprint('access-token')).toMatch(/^sha256:[0-9a-f]{16}$/)
    expect(tokenFingerprint('access-token')).not.toContain('access-token')
    expect(tokenFingerprint('access-token')).not.toBe(tokenFingerprint('other-token'))
  })

  it('refreshes a local session at most once at a time', async () => {
    const sessions = new SessionStore(() => 1_000)
    const profile = new ProfileDirectory().upsert(
      {
        issuer: 'https://id.example.com',
        subject: 'user-1',
        displayName: 'Alice',
        audience: [config.clientId],
      },
      1_000,
    )
    const session = sessions.create(
      profile,
      {
        accessToken: 'old-access',
        refreshToken: 'old-refresh',
        expiresAt: 2_000,
        accessTokenFingerprint: tokenFingerprint('old-access'),
      },
      toProtocolEvidence(
        {
          issuer: 'https://id.example.com',
          subject: 'user-1',
          displayName: 'Alice',
          sid: 'sid-1',
          audience: [config.clientId],
        },
        config.clientId,
      ),
    )

    let release: (() => void) | undefined
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    const refresher = vi.fn(async () => {
      await gate
      return {
        accessToken: 'new-access',
        refreshToken: 'new-refresh',
        expiresAt: 9_000,
        accessTokenFingerprint: tokenFingerprint('new-access'),
      }
    })

    const first = sessions.refresh(session.id, refresher)
    const second = sessions.refresh(session.id, refresher)
    release?.()
    const [a, b] = await Promise.all([first, second])

    expect(refresher).toHaveBeenCalledTimes(1)
    expect(a.tokens.accessToken).toBe('new-access')
    expect(b.tokens.accessToken).toBe('new-access')
    expect(sessions.get(session.id)?.tokens.refreshToken).toBe('new-refresh')
  })

  it('removes local sessions by central session id', () => {
    const sessions = new SessionStore()
    const profile = new ProfileDirectory().upsert(
      {
        issuer: 'https://id.example.com',
        subject: 'user-1',
        displayName: 'Alice',
        audience: [config.clientId],
      },
      1_000,
    )
    const tokens = { accessToken: 'a', expiresAt: 9_000, accessTokenFingerprint: 'sha256:a' }
    const protocol = (sid: string) =>
      toProtocolEvidence(
        {
          issuer: 'https://id.example.com',
          subject: 'user-1',
          displayName: 'Alice',
          sid,
          audience: [config.clientId],
        },
        config.clientId,
      )
    const kept = sessions.create(profile, tokens, protocol('sid-1'))
    const dropped = sessions.create(profile, tokens, protocol('sid-2'))

    expect(sessions.deleteBySid('sid-2')).toBe(1)
    expect(sessions.get(kept.id)).toBeDefined()
    expect(sessions.get(dropped.id)).toBeUndefined()
  })

  it('marks cookies HttpOnly and Secure for an HTTPS issuer', () => {
    expect(sessionCookie('abc', true)).toContain('HttpOnly')
    expect(sessionCookie('abc', true)).toContain('Secure')
    expect(sessionCookie('abc', true)).toContain('SameSite=Lax')
    expect(pendingCookie('state', false)).not.toContain('Secure')
  })
})
