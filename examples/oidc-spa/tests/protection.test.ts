import { describe, expect, it } from 'vitest'

import {
  AuthorizationCache,
  authorizeProtectedOperation,
  OperationDenied,
  StaleAuthorization,
  type AccessTokenClaims,
  type CacheEntry,
  type IntrospectionResult,
} from '../server/protection.js'

const ISSUER = 'https://id.example.com'
const CLIENT_ID = 'yufu-spa-example'
const WALL = 1_000_000

function claims(overrides: Partial<AccessTokenClaims> = {}): AccessTokenClaims {
  return {
    issuer: ISSUER,
    subject: 'user-1',
    audience: 'urn:app:production',
    expiresAt: WALL + 300_000,
    sessionId: 'credential-session-1',
    application: CLIENT_ID,
    scope: 'openid profile',
    jti: 'access-jti-1',
    ...overrides,
  }
}

function activeResult(exp = Math.floor(WALL / 1000) + 300): IntrospectionResult {
  return {
    active: true,
    sub: 'user-1',
    clientId: CLIENT_ID,
    exp,
    sid: 'protocol-session-1',
  }
}

interface Clock {
  mono: number
  wall: number
}

function newCache(clock: Clock): AuthorizationCache {
  return new AuthorizationCache({
    monotonic: () => clock.mono,
    wallClock: () => clock.wall,
  })
}

describe('protected operation cache windows', () => {
  it('serves a positive result for two seconds and then re-introspects', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    let calls = 0
    const introspector = {
      introspect: async () => {
        calls += 1
        return activeResult()
      },
    }
    const input = {
      token: 'access-token-1',
      claims: claims(),
      cache,
      introspector,
      expectedClientId: CLIENT_ID,
    }

    await authorizeProtectedOperation(input)
    clock.mono += 1_999
    await authorizeProtectedOperation(input)
    expect(calls).toBe(1)

    clock.mono += 2
    await authorizeProtectedOperation(input)
    expect(calls).toBe(2)
  })

  it('accepts a forwarded result up to the five second hard cap', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const entry = await authorizeProtectedOperation({
      token: 'access-token-1',
      claims: claims(),
      cache,
      introspector: { introspect: async () => activeResult() },
      expectedClientId: CLIENT_ID,
    })

    clock.mono += 5_000
    expect(cache.acceptForwarded(entry)).toBe(true)

    clock.mono += 1
    expect(cache.acceptForwarded(entry)).toBe(false)
  })

  it('keeps the original verification instant when a result is forwarded', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const entry = await authorizeProtectedOperation({
      token: 'access-token-1',
      claims: claims(),
      cache,
      introspector: { introspect: async () => activeResult() },
      expectedClientId: CLIENT_ID,
    })

    clock.mono += 4_000
    const forwarded = await authorizeProtectedOperation({
      token: 'access-token-1',
      claims: claims(),
      cache,
      introspector: {
        introspect: async () => {
          throw new Error('must not be called')
        },
      },
      expectedClientId: CLIENT_ID,
      forwarded: entry,
    })

    expect(forwarded).toBe(entry)
    expect(forwarded.verifiedAt).toBe(1_000)
    expect(cache.acceptForwarded(forwarded)).toBe(true)
  })

  it('does not extend freshness when introspection is slow', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)

    const entry = await authorizeProtectedOperation({
      token: 'access-token-1',
      claims: claims(),
      cache,
      introspector: {
        introspect: async () => {
          clock.mono += 4_000
          return activeResult()
        },
      },
      expectedClientId: CLIENT_ID,
    })

    expect(entry.verifiedAt).toBe(1_000)
    expect(cache.get(AuthorizationCache.key('access-token-1'))).toBeUndefined()
    expect(cache.acceptForwarded(entry)).toBe(true)

    clock.mono += 1_001
    expect(cache.acceptForwarded(entry)).toBe(false)
  })

  it('fails closed when introspection exceeds the hard cap', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)

    await expect(
      authorizeProtectedOperation({
        token: 'access-token-1',
        claims: claims(),
        cache,
        introspector: {
          introspect: async () => {
            clock.mono += 6_000
            return activeResult()
          },
        },
        expectedClientId: CLIENT_ID,
      }),
    ).rejects.toMatchObject({ reason: 'stale_authorization' })
  })

  it('never stores or serves a result whose token already expired', () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const entry: CacheEntry = {
      allowed: true,
      subject: 'user-1',
      clientId: CLIENT_ID,
      verifiedAt: clock.mono,
      tokenExpiresAt: clock.wall,
    }

    expect(cache.put('key', entry)).toBe(false)
    expect(cache.get('key')).toBeUndefined()
    expect(cache.acceptForwarded(entry)).toBe(false)
    expect(() => cache.assertFresh(entry)).toThrow(StaleAuthorization)
  })
})

describe('protected operation cache invalidation', () => {
  it('clears entries by sid and by subject', () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const base: CacheEntry = {
      allowed: true,
      subject: 'user-1',
      sid: 'protocol-session-1',
      clientId: CLIENT_ID,
      verifiedAt: clock.mono,
      tokenExpiresAt: clock.wall + 300_000,
    }
    cache.put('a', base)
    cache.put('b', { ...base, sid: 'protocol-session-2' })
    cache.put('c', { ...base, subject: 'user-2', sid: 'protocol-session-3' })

    expect(cache.clearForLogout('protocol-session-1')).toBe(1)
    expect(cache.size).toBe(2)
    expect(cache.clearForLogout(undefined, 'user-2')).toBe(1)
    expect(cache.size).toBe(1)
  })

  it('ignores a forwarded result that names another client or subject', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const forwarded: CacheEntry = {
      allowed: true,
      subject: 'user-2',
      clientId: CLIENT_ID,
      verifiedAt: clock.mono,
      tokenExpiresAt: clock.wall + 300_000,
    }
    let calls = 0

    const entry = await authorizeProtectedOperation({
      token: 'access-token-1',
      claims: claims(),
      cache,
      introspector: {
        introspect: async () => {
          calls += 1
          return activeResult()
        },
      },
      expectedClientId: CLIENT_ID,
      forwarded,
    })

    expect(calls).toBe(1)
    expect(entry.subject).toBe('user-1')
  })

  it('reports a stale authorization as a denial reason', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)

    const error = await authorizeProtectedOperation({
      token: 'access-token-1',
      claims: claims(),
      cache,
      introspector: {
        introspect: async () => {
          clock.mono += 6_000
          return activeResult()
        },
      },
      expectedClientId: CLIENT_ID,
    }).catch((reason: unknown) => reason)

    expect(error).toBeInstanceOf(OperationDenied)
    expect((error as OperationDenied).reason).toBe('stale_authorization')
  })
})
