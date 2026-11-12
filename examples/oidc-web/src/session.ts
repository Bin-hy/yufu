import { randomBytes } from 'node:crypto'

import type { IdentityClaims, TokenSet, VerifiedProtocol } from './oidc.js'

export const SESSION_COOKIE = 'cobotrix_session'
export const PENDING_COOKIE = 'cobotrix_pending'

const SESSION_COOKIE_MAX_AGE = 8 * 60 * 60
const PENDING_COOKIE_MAX_AGE = 10 * 60

/** Minimal local business profile, keyed by the pair (iss, sub). */
export interface LocalProfile {
  issuer: string
  subject: string
  displayName: string
  email?: string
  createdAt: number
  lastLoginAt: number
}

function profileKey(issuer: string, subject: string): string {
  return `${issuer}\u0000${subject}`
}

export class ProfileDirectory {
  #profiles = new Map<string, LocalProfile>()

  upsert(claims: IdentityClaims, now: number): LocalProfile {
    const key = profileKey(claims.issuer, claims.subject)
    const existing = this.#profiles.get(key)
    const profile: LocalProfile = existing
      ? { ...existing, displayName: claims.displayName, email: claims.email, lastLoginAt: now }
      : {
          issuer: claims.issuer,
          subject: claims.subject,
          displayName: claims.displayName,
          email: claims.email,
          createdAt: now,
          lastLoginAt: now,
        }
    this.#profiles.set(key, profile)
    return profile
  }

  get(issuer: string, subject: string): LocalProfile | undefined {
    return this.#profiles.get(profileKey(issuer, subject))
  }

  get size(): number {
    return this.#profiles.size
  }
}

export interface SessionRecord {
  id: string
  profile: LocalProfile
  tokens: TokenSet
  protocol: VerifiedProtocol
  createdAt: number
}

export function newSessionId(): string {
  return randomBytes(32).toString('base64url')
}

export class SessionStore {
  #sessions = new Map<string, SessionRecord>()
  #inflight = new Map<string, Promise<SessionRecord>>()

  constructor(private readonly clock: () => number = Date.now) {}

  create(profile: LocalProfile, tokens: TokenSet, protocol: VerifiedProtocol): SessionRecord {
    const record: SessionRecord = {
      id: newSessionId(),
      profile,
      tokens,
      protocol,
      createdAt: this.clock(),
    }
    this.#sessions.set(record.id, record)
    return record
  }

  get(id: string): SessionRecord | undefined {
    return this.#sessions.get(id)
  }

  delete(id: string): boolean {
    this.#inflight.delete(id)
    return this.#sessions.delete(id)
  }

  /** Removes every local session bound to a central session id (backchannel). */
  deleteBySid(sid: string): number {
    let removed = 0
    for (const [id, record] of this.#sessions) {
      if (record.protocol.sid === sid) {
        this.#sessions.delete(id)
        this.#inflight.delete(id)
        removed += 1
      }
    }
    return removed
  }

  /** Removes every local session for one (iss, sub) pair. */
  deleteBySubject(issuer: string, subject: string): number {
    let removed = 0
    for (const [id, record] of this.#sessions) {
      if (record.profile.issuer === issuer && record.profile.subject === subject) {
        this.#sessions.delete(id)
        this.#inflight.delete(id)
        removed += 1
      }
    }
    return removed
  }

  /**
   * Refreshes one local session at most once at a time. Concurrent callers
   * share the same in-flight promise, so a burst of requests cannot rotate the
   * refresh token twice.
   */
  async refresh(
    id: string,
    refresher: (record: SessionRecord) => Promise<TokenSet>,
  ): Promise<SessionRecord> {
    const running = this.#inflight.get(id)
    if (running) return running

    const record = this.#sessions.get(id)
    if (!record) throw new Error('session not found')

    const task = (async () => {
      const tokens = await refresher(record)
      const updated: SessionRecord = { ...record, tokens }
      this.#sessions.set(id, updated)
      return updated
    })()

    this.#inflight.set(id, task)
    try {
      return await task
    } finally {
      this.#inflight.delete(id)
    }
  }
}

export interface PendingAuthorization {
  state: string
  nonce: string
  codeVerifier: string
  returnTo: string
  expiresAt: number
}

export class PendingStore {
  #pending = new Map<string, PendingAuthorization>()

  constructor(
    private readonly ttlMs: number = PENDING_COOKIE_MAX_AGE * 1000,
    private readonly clock: () => number = Date.now,
  ) {}

  put(
    pending: Omit<PendingAuthorization, 'expiresAt'>,
  ): PendingAuthorization {
    const record: PendingAuthorization = {
      ...pending,
      expiresAt: this.clock() + this.ttlMs,
    }
    this.#pending.set(record.state, record)
    return record
  }

  /** Single use: a state value can complete exactly one authorization. */
  consume(state: string): PendingAuthorization | undefined {
    const record = this.#pending.get(state)
    if (!record) return undefined
    this.#pending.delete(state)
    if (record.expiresAt <= this.clock()) return undefined
    return record
  }

  get size(): number {
    return this.#pending.size
  }
}

/** Only same-origin absolute paths are accepted as a post-login destination. */
export function safeReturnTo(value: string | undefined | null): string {
  if (!value) return '/'
  if (!value.startsWith('/') || value.startsWith('//') || value.includes('\\')) return '/'
  return value
}

function cookieAttributes(secure: boolean, maxAge: number): string {
  const parts = ['Path=/', 'HttpOnly', 'SameSite=Lax', `Max-Age=${maxAge}`]
  if (secure) parts.push('Secure')
  return parts.join('; ')
}

export function sessionCookie(id: string, secure: boolean): string {
  return `${SESSION_COOKIE}=${id}; ${cookieAttributes(secure, SESSION_COOKIE_MAX_AGE)}`
}

export function pendingCookie(state: string, secure: boolean): string {
  return `${PENDING_COOKIE}=${state}; ${cookieAttributes(secure, PENDING_COOKIE_MAX_AGE)}`
}

export function clearCookie(name: string, secure: boolean): string {
  return `${name}=; ${cookieAttributes(secure, 0)}`
}
