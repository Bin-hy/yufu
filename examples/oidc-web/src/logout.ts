import { randomBytes } from 'node:crypto'

import { createRemoteJWKSet, jwtVerify, type CryptoKey, type JWTPayload, type JWTVerifyGetKey } from 'jose'

import type { SessionStore } from './session.js'

export const BACKCHANNEL_LOGOUT_EVENT =
  'http://schemas.openid.net/event/backchannel-logout'

export class LogoutTokenError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'LogoutTokenError'
  }
}

export interface VerifiedLogoutToken {
  issuer: string
  clientId: string
  subject?: string
  sid?: string
  jti: string
  issuedAt: number
}

export type LogoutVerificationKey = CryptoKey | Uint8Array | JWTVerifyGetKey

export interface LogoutTokenVerifierOptions {
  issuer: string
  clientId: string
  /** Defaults to the issuer's published JWKS endpoint. */
  key?: LogoutVerificationKey
  now?: () => number
  clockToleranceSeconds?: number
}

function stringClaim(payload: JWTPayload, name: string): string | undefined {
  const value = payload[name]
  return typeof value === 'string' && value ? value : undefined
}

/**
 * Verifies an OIDC Back-Channel Logout token: signature, issuer, audience,
 * `typ`, lifetime, the single backchannel event, the absence of `nonce`, and
 * the presence of `jti` plus `sid` or `sub`.
 */
export function createLogoutTokenVerifier(
  options: LogoutTokenVerifierOptions,
): (token: string) => Promise<VerifiedLogoutToken> {
  const now = options.now ?? Date.now
  const tolerance = options.clockToleranceSeconds ?? 60
  const key =
    options.key ?? createRemoteJWKSet(new URL(`${options.issuer}/.well-known/jwks.json`))

  return async (token) => {
    let payload: JWTPayload
    try {
      const result = await jwtVerify(token, key, {
        issuer: options.issuer,
        audience: options.clientId,
        typ: 'logout+jwt',
        algorithms: ['RS256'],
        clockTolerance: tolerance,
        maxTokenAge: 300,
        currentDate: new Date(now()),
      })
      payload = result.payload
    } catch (error) {
      throw new LogoutTokenError(
        `invalid logout token: ${error instanceof Error ? error.message : 'verification failed'}`,
      )
    }

    const events = payload.events
    const hasEvent =
      typeof events === 'object' &&
      events !== null &&
      !Array.isArray(events) &&
      Object.hasOwn(events, BACKCHANNEL_LOGOUT_EVENT)
    if (!hasEvent) throw new LogoutTokenError('logout token is missing the backchannel event')

    if (payload.nonce !== undefined) {
      throw new LogoutTokenError('logout token must not contain nonce')
    }

    const jti = stringClaim(payload, 'jti')
    if (!jti) throw new LogoutTokenError('logout token is missing jti')

    const sid = stringClaim(payload, 'sid')
    const subject = stringClaim(payload, 'sub')
    if (!sid && !subject) {
      throw new LogoutTokenError('logout token must identify sid or sub')
    }

    return {
      issuer: payload.iss as string,
      clientId: Array.isArray(payload.aud) ? (payload.aud[0] as string) : (payload.aud as string),
      subject,
      sid,
      jti,
      issuedAt: typeof payload.iat === 'number' ? payload.iat : 0,
    }
  }
}

/** Bounded replay cache: a repeated jti is acknowledged but not re-applied. */
export class LogoutReplayGuard {
  #seen = new Map<string, number>()

  constructor(
    private readonly ttlMs: number = 10 * 60 * 1000,
    private readonly clock: () => number = Date.now,
  ) {}

  /** Returns true when this jti was already processed. */
  seen(jti: string): boolean {
    const now = this.clock()
    for (const [key, expiresAt] of this.#seen) {
      if (expiresAt <= now) this.#seen.delete(key)
    }
    if (this.#seen.has(jti)) return true
    this.#seen.set(jti, now + this.ttlMs)
    return false
  }
}

/** Clears every local session named by a verified notification. */
export function applyBackchannelLogout(
  sessions: SessionStore,
  token: VerifiedLogoutToken,
): number {
  if (token.sid) return sessions.deleteBySid(token.sid)
  if (token.subject) return sessions.deleteBySubject(token.issuer, token.subject)
  return 0
}

export function newLogoutState(): string {
  return randomBytes(16).toString('base64url')
}

export interface EndSessionInput {
  endSessionEndpoint: string
  idTokenHint?: string
  postLogoutRedirectUri: string
  state: string
}

/** Builds a standard RP-initiated logout request. */
export function buildEndSessionUrl(input: EndSessionInput): string {
  const url = new URL(input.endSessionEndpoint)
  if (input.idTokenHint) url.searchParams.set('id_token_hint', input.idTokenHint)
  url.searchParams.set('post_logout_redirect_uri', input.postLogoutRedirectUri)
  url.searchParams.set('state', input.state)
  return url.href
}
