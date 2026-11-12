import { describe, expect, it } from 'vitest'

import { tokenFingerprint, type VerifiedProtocol } from '../src/oidc.js'

import {
  AuthorizationCache,
  authorizeOnlineOperation,
  OperationDenied,
  StaleAuthorization,
  type CacheEntry,
  type IntrospectionResult,
} from '../src/protection.js'
import { ProfileDirectory, SessionStore } from '../src/session.js'

const ISSUER = 'https://id.example.com'
const CLIENT_ID = 'yufu-web-example'
const WALL = 1_000_000

function protocol(): VerifiedProtocol {
  return {
    issuer: ISSUER,
    subject: 'user-1',
    clientId: CLIENT_ID,
    audience: [CLIENT_ID],
    sid: 'sid-1',
    authTime: 1,
    stateVerified: true,
    nonceVerified: true,
  }
}

function newSession(accessToken = 'access-token-1', expiresAt = WALL + 300_000) {
  const profile = new ProfileDirectory().upsert(
    { issuer: ISSUER, subject: 'user-1', displayName: 'Alice', audience: [CLIENT_ID] },
    0,
  )
  const sessions = new SessionStore(() => 0)
  return sessions.create(profile, { accessToken, expiresAt, accessTokenFingerprint: tokenFingerprint(accessToken) }, protocol())
}

function activeResult(exp = Math.floor(WALL / 1000) + 300): IntrospectionResult {
  return { active: true, clientId: CLIENT_ID, sub: 'user-1', exp }
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

describe('authorization cache windows', () => {
  it('serves a positive result for two seconds and then re-introspects', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const session = newSession()
    let calls = 0
    const introspector = {
      introspect: async () => {
        calls += 1
        return activeResult()
      },
    }

    await authorizeOnlineOperation({
      session,
      introspector,
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
    })
    clock.mono += 1_999
    await authorizeOnlineOperation({
      session,
      introspector,
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
    })
    expect(calls).toBe(1)

    clock.mono += 2
    await authorizeOnlineOperation({
      session,
      introspector,
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
    })
    expect(calls).toBe(2)
  })

  it('accepts a forwarded result up to the five second hard cap', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const session = newSession()
    const entry = await authorizeOnlineOperation({
      session,
      introspector: { introspect: async () => activeResult() },
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
    })

    clock.mono += 5_000
    expect(cache.acceptForwarded(entry)).toBe(true)

    clock.mono += 1
    expect(cache.acceptForwarded(entry)).toBe(false)
  })

  it('keeps the original verification instant when a result is forwarded', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const session = newSession()
    const entry = await authorizeOnlineOperation({
      session,
      introspector: { introspect: async () => activeResult() },
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
    })

    clock.mono += 4_000
    const forwarded = await authorizeOnlineOperation({
      session,
      introspector: {
        introspect: async () => {
          throw new Error('must not be called')
        },
      },
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
      forwarded: entry,
    })

    expect(forwarded).toBe(entry)
    expect(forwarded.verifiedAt).toBe(1_000)
    expect(cache.acceptForwarded(forwarded)).toBe(true)
  })

  it('does not extend freshness when introspection is slow', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const session = newSession()

    const entry = await authorizeOnlineOperation({
      session,
      introspector: {
        introspect: async () => {
          clock.mono += 4_000
          return activeResult()
        },
      },
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
    })

    expect(entry.verifiedAt).toBe(1_000)
    expect(cache.get(AuthorizationCache.key(session.tokens.accessToken))).toBeUndefined()
    expect(cache.acceptForwarded(entry)).toBe(true)

    clock.mono += 1_001
    expect(cache.acceptForwarded(entry)).toBe(false)
  })

  it('fails closed when introspection exceeds the hard cap', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const session = newSession()

    await expect(
      authorizeOnlineOperation({
        session,
        introspector: {
          introspect: async () => {
            clock.mono += 6_000
            return activeResult()
          },
        },
        expectedClientId: CLIENT_ID,
        now: clock.wall,
        cache,
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

describe('authorization cache invalidation', () => {
  it('clears entries by sid and by subject', () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const base: CacheEntry = {
      allowed: true,
      subject: 'user-1',
      sid: 'sid-1',
      clientId: CLIENT_ID,
      verifiedAt: clock.mono,
      tokenExpiresAt: clock.wall + 300_000,
    }
    cache.put('a', base)
    cache.put('b', { ...base, sid: 'sid-2' })
    cache.put('c', { ...base, subject: 'user-2', sid: 'sid-3' })

    expect(cache.clearForLogout('sid-1')).toBe(1)
    expect(cache.size).toBe(2)
    expect(cache.clearForLogout(undefined, 'user-2')).toBe(1)
    expect(cache.size).toBe(1)
  })

  it('ignores a forwarded result that names another client or subject', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const session = newSession()
    const forwarded: CacheEntry = {
      allowed: true,
      subject: 'user-2',
      clientId: CLIENT_ID,
      verifiedAt: clock.mono,
      tokenExpiresAt: clock.wall + 300_000,
    }
    let calls = 0

    const entry = await authorizeOnlineOperation({
      session,
      introspector: {
        introspect: async () => {
          calls += 1
          return activeResult()
        },
      },
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
      forwarded,
    })

    expect(calls).toBe(1)
    expect(entry.subject).toBe('user-1')
  })

  it('reports a stale authorization as a denial reason', async () => {
    const clock: Clock = { mono: 1_000, wall: WALL }
    const cache = newCache(clock)
    const session = newSession()

    const error = await authorizeOnlineOperation({
      session,
      introspector: {
        introspect: async () => {
          clock.mono += 6_000
          return activeResult()
        },
      },
      expectedClientId: CLIENT_ID,
      now: clock.wall,
      cache,
    }).catch((reason: unknown) => reason)

    expect(error).toBeInstanceOf(OperationDenied)
    expect((error as OperationDenied).reason).toBe('stale_authorization')
  })
})
