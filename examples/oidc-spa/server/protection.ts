import { createHash } from 'node:crypto'

import {
  createRemoteJWKSet,
  jwtVerify,
  type CryptoKey,
  type JWTPayload,
  type JWTVerifyGetKey,
} from 'jose'

import type { ApiConfig } from './config.js'

export type AccessTokenVerificationKey = CryptoKey | Uint8Array | JWTVerifyGetKey

export interface AccessTokenClaims {
  issuer: string
  subject: string
  audience: string
  expiresAt: number
  sessionId?: string
  application?: string
  scope?: string
  jti?: string
}

export interface AccessTokenVerifierOptions {
  issuer: string
  /** The application's registered access audience. */
  audience: string
  key?: AccessTokenVerificationKey
  now?: () => number
  clockToleranceSeconds?: number
}

export class AccessTokenError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'AccessTokenError'
  }
}

function stringClaim(payload: JWTPayload, name: string): string | undefined {
  const value = payload[name]
  return typeof value === 'string' && value ? value : undefined
}

/**
 * Local bearer verification: RS256 signature from the issuer JWKS, issuer,
 * audience and lifetime, and `typ: at+jwt` so an ID Token can never be replayed
 * as an access token.
 */
export function createAccessTokenVerifier(
  options: AccessTokenVerifierOptions,
): (token: string) => Promise<AccessTokenClaims> {
  const now = options.now ?? Date.now
  const key =
    options.key ?? createRemoteJWKSet(new URL(`${options.issuer}/.well-known/jwks.json`))

  return async (token) => {
    let payload: JWTPayload
    try {
      const result = await jwtVerify(token, key, {
        issuer: options.issuer,
        audience: options.audience,
        typ: 'at+jwt',
        algorithms: ['RS256'],
        clockTolerance: options.clockToleranceSeconds ?? 30,
        currentDate: new Date(now()),
      })
      payload = result.payload
    } catch (error) {
      throw new AccessTokenError(
        `invalid access token: ${error instanceof Error ? error.message : 'verification failed'}`,
      )
    }

    const subject = stringClaim(payload, 'sub')
    if (!subject) throw new AccessTokenError('access token is missing sub')
    const audience = Array.isArray(payload.aud) ? payload.aud[0] : payload.aud
    if (typeof audience !== 'string') throw new AccessTokenError('access token is missing aud')

    return {
      issuer: payload.iss as string,
      subject,
      audience,
      expiresAt: typeof payload.exp === 'number' ? payload.exp * 1000 : 0,
      sessionId: stringClaim(payload, 'session_id'),
      application: stringClaim(payload, 'application'),
      scope: stringClaim(payload, 'scope'),
      jti: stringClaim(payload, 'jti'),
    }
  }
}

export interface IntrospectionResult {
  active: boolean
  sub?: string
  clientId?: string
  scope?: string
  exp?: number
  sid?: string
}

export interface Introspector {
  introspect(token: string): Promise<IntrospectionResult>
}

export interface IntrospectorOptions {
  endpoint?: string
  fetch?: typeof fetch
}

/** Reader-authenticated introspection; the reader secret stays in this process. */
export function createIntrospector(
  config: ApiConfig,
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
        sid: typeof payload.sid === 'string' ? payload.sid : undefined,
      }
    },
  }
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

  #age(entry: CacheEntry): number {
    return this.#monotonic() - entry.verifiedAt
  }

  #usable(entry: CacheEntry, maxAgeMs: number): boolean {
    const age = this.#age(entry)
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

export type OperationDenial =
  | 'invalid_token'
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

/** Only the registered SPA origin may call the API from a browser. */
export function originAllowed(origin: string | undefined, expected: string): boolean {
  if (!origin) return false
  try {
    return new URL(origin).origin === new URL(expected).origin
  } catch {
    return false
  }
}

export interface ProtectedOperationInput {
  token: string
  claims: AccessTokenClaims
  cache: AuthorizationCache
  introspector: Introspector
  expectedClientId: string
  forwarded?: CacheEntry
}

/**
 * Local JWT checks are already done by the verifier. This adds the online
 * admission check: the token must still be active, must belong to this client
 * and must belong to the subject named by the JWT.
 */
export async function authorizeProtectedOperation(
  input: ProtectedOperationInput,
): Promise<CacheEntry> {
  const key = AuthorizationCache.key(input.token)

  if (input.forwarded && input.cache.acceptForwarded(input.forwarded)) {
    if (
      input.forwarded.subject === input.claims.subject &&
      input.forwarded.clientId === input.expectedClientId
    ) {
      return input.forwarded
    }
  }

  const cached = input.cache.get(key)
  if (cached) return cached

  const verifiedAt = input.cache.now()
  let result: IntrospectionResult
  try {
    result = await input.introspector.introspect(input.token)
  } catch {
    throw new OperationDenied('introspection_unavailable')
  }

  if (!result.active) throw new OperationDenied('inactive_token')
  if (result.clientId !== input.expectedClientId) throw new OperationDenied('wrong_client')
  if (result.sub !== input.claims.subject) throw new OperationDenied('wrong_subject')

  const tokenExpiresAt = result.exp !== undefined ? result.exp * 1000 : input.claims.expiresAt
  if (tokenExpiresAt <= input.cache.wallNow()) throw new OperationDenied('expired_token')

  const entry: CacheEntry = {
    allowed: true,
    subject: input.claims.subject,
    sid: result.sid,
    clientId: input.expectedClientId,
    verifiedAt,
    tokenExpiresAt,
  }
  if (!input.cache.put(key, entry)) throw new OperationDenied('stale_authorization')
  return entry
}
