import { createHash, createHmac, timingSafeEqual } from 'node:crypto'

import type { WebConfig } from './config.js'
import type { SessionRecord } from './session.js'

export interface IntrospectionResult {
  active: boolean
  sub?: string
  clientId?: string
  scope?: string
  exp?: number
}

export interface Introspector {
  introspect(token: string): Promise<IntrospectionResult>
}

export interface IntrospectorOptions {
  endpoint?: string
  fetch?: typeof fetch
}

/** Reader-authenticated introspection. The reader secret never leaves the server. */
export function createIntrospector(
  config: WebConfig,
  options: IntrospectorOptions = {},
): Introspector {
  const endpoint = options.endpoint ?? `${config.issuer}/oidc/introspect`
  const http = options.fetch ?? fetch
  const basic = Buffer.from(
    `${config.readerClientId}:${config.readerClientSecret}`,
  ).toString('base64')

  return {
    async introspect(token) {
      const response = await http(endpoint, {
        method: 'POST',
        headers: {
          authorization: `Basic ${basic}`,
          'content-type': 'application/x-www-form-urlencoded',
        },
        body: new URLSearchParams({ token, token_type_hint: 'access_token' }),
      })
      if (!response.ok) {
        throw new Error(`introspection failed with status ${response.status}`)
      }
      const payload = (await response.json()) as Record<string, unknown>
      return {
        active: payload.active === true,
        sub: typeof payload.sub === 'string' ? payload.sub : undefined,
        clientId: typeof payload.client_id === 'string' ? payload.client_id : undefined,
        scope: typeof payload.scope === 'string' ? payload.scope : undefined,
        exp: typeof payload.exp === 'number' ? payload.exp : undefined,
      }
    },
  }
}

export type OperationDenial =
  | 'unauthenticated'
  | 'inactive_token'
  | 'wrong_client'
  | 'wrong_subject'
  | 'expired_token'
  | 'introspection_unavailable'
  | 'stale_authorization'

export class OperationDenied extends Error {
  constructor(readonly reason: OperationDenial) {
    super(`operation denied: ${reason}`)
    this.name = 'OperationDenied'
  }
}

export interface OperationAuthorizationInput {
  session: SessionRecord | undefined
  introspector: Introspector
  expectedClientId: string
  now: number
  cache?: AuthorizationCache
  forwarded?: CacheEntry
}

export interface CacheEntry {
  allowed: boolean
  subject: string
  sid?: string
  clientId?: string
  /** Monotonic instant at which the introspection request started. */
  verifiedAt: number
  /** Wall-clock token expiry in epoch milliseconds. */
  tokenExpiresAt: number
}

export interface AuthorizationCacheOptions {
  /** Fresh window for locally cached positive results (default 2 s). */
  positiveTtlMs?: number
  /** Absolute age a result may reach, including forwarded results (default 5 s). */
  maxAgeMs?: number
  monotonic?: () => number
  wallClock?: () => number
}

export class StaleAuthorization extends Error {
  constructor(message = 'authorization is stale') {
    super(message)
    this.name = 'StaleAuthorization'
  }
}

/**
 * Positive-only authorization cache.
 *
 * Age is measured from the start of the introspection call on a monotonic
 * clock, so a slow introspection cannot buy extra freshness. Results older
 * than the hard cap are never stored, and a result forwarded from another
 * layer keeps its original verification instant instead of restarting its TTL.
 */
export class AuthorizationCache {
  #entries = new Map<string, CacheEntry>()
  readonly #positiveTtlMs: number
  readonly #maxAgeMs: number
  readonly #monotonic: () => number
  readonly #wallClock: () => number

  constructor(options: AuthorizationCacheOptions = {}) {
    this.#positiveTtlMs = options.positiveTtlMs ?? 2_000
    this.#maxAgeMs = options.maxAgeMs ?? 5_000
    this.#monotonic = options.monotonic ?? (() => performance.now())
    this.#wallClock = options.wallClock ?? Date.now
  }

  static key(token: string): string {
    return createHash('sha256').update(token).digest('base64url')
  }

  #usable(entry: CacheEntry, maxAgeMs: number): boolean {
    const age = this.#monotonic() - entry.verifiedAt
    if (!Number.isFinite(age) || age < 0 || age > maxAgeMs) return false
    return entry.tokenExpiresAt > this.#wallClock()
  }

  /** A locally cached entry is only served inside the positive window. */
  get(key: string): CacheEntry | undefined {
    const entry = this.#entries.get(key)
    if (!entry) return undefined
    if (!this.#usable(entry, this.#positiveTtlMs)) {
      this.#entries.delete(key)
      return undefined
    }
    return entry
  }

  /** A result forwarded between layers may be used up to the hard cap. */
  acceptForwarded(entry: CacheEntry): boolean {
    return this.#usable(entry, this.#maxAgeMs)
  }

  /** Over-age or already-expired results are not cached. */
  put(key: string, entry: CacheEntry): boolean {
    if (!this.#usable(entry, this.#maxAgeMs)) return false
    this.#entries.set(key, entry)
    return true
  }

  /** Re-checked immediately before the protected side effect. */
  assertFresh(entry: CacheEntry): void {
    if (!this.#usable(entry, this.#maxAgeMs)) throw new StaleAuthorization()
  }

  clearForLogout(sid?: string, subject?: string): number {
    let removed = 0
    for (const [key, entry] of this.#entries) {
      if ((sid && entry.sid === sid) || (subject && entry.subject === subject)) {
        this.#entries.delete(key)
        removed += 1
      }
    }
    return removed
  }

  get size(): number {
    return this.#entries.size
  }

  /** Monotonic instant used to age results. */
  now(): number {
    return this.#monotonic()
  }

  /** Wall-clock instant used to compare token expiry. */
  wallNow(): number {
    return this.#wallClock()
  }
}

/**
 * Every protected write is authorized online: the server-held access token must
 * still be active, must belong to this application, and must belong to the
 * subject of the local session.
 */
export async function authorizeOnlineOperation(
  input: OperationAuthorizationInput,
): Promise<CacheEntry> {
  if (!input.session) throw new OperationDenied('unauthenticated')

  const token = input.session.tokens.accessToken
  const cache = input.cache

  if (cache) {
    if (input.forwarded && cache.acceptForwarded(input.forwarded)) {
      if (
        input.forwarded.subject === input.session.profile.subject &&
        input.forwarded.clientId === input.expectedClientId
      ) {
        return input.forwarded
      }
    }
    const cached = cache.get(AuthorizationCache.key(token))
    if (cached) return cached
  }

  const verifiedAt = cache ? cache.now() : input.now
  let result: IntrospectionResult
  try {
    result = await input.introspector.introspect(token)
  } catch {
    throw new OperationDenied('introspection_unavailable')
  }

  if (!result.active) throw new OperationDenied('inactive_token')
  if (result.clientId !== input.expectedClientId) throw new OperationDenied('wrong_client')
  if (result.sub !== input.session.profile.subject) throw new OperationDenied('wrong_subject')
  if (result.exp !== undefined && result.exp * 1000 <= input.now) {
    throw new OperationDenied('expired_token')
  }

  const entry: CacheEntry = {
    allowed: true,
    subject: input.session.profile.subject,
    sid: input.session.sid,
    clientId: input.expectedClientId,
    verifiedAt,
    tokenExpiresAt: result.exp !== undefined ? result.exp * 1000 : input.session.tokens.expiresAt,
  }
  if (cache) {
    if (!cache.put(AuthorizationCache.key(token), entry)) {
      throw new OperationDenied('stale_authorization')
    }
    cache.assertFresh(entry)
  }
  return entry
}

/** HMAC token bound to the local session id (double-submit CSRF defence). */
export function csrfToken(sessionId: string, secret: string): string {
  return createHmac('sha256', secret).update(sessionId).digest('base64url')
}

export function verifyCsrfToken(
  sessionId: string,
  secret: string,
  candidate: string | undefined,
): boolean {
  if (!candidate) return false
  const expected = Buffer.from(csrfToken(sessionId, secret))
  const actual = Buffer.from(candidate)
  return expected.length === actual.length && timingSafeEqual(expected, actual)
}

/** Rejects cross-site form posts before any state is read. */
export function originAllowed(origin: string | undefined, expected: string): boolean {
  if (!origin) return false
  try {
    return new URL(origin).origin === new URL(expected).origin
  } catch {
    return false
  }
}
