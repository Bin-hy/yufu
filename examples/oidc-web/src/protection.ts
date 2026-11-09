import { createHmac, timingSafeEqual } from 'node:crypto'

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
}

/**
 * Every protected write is authorized online: the server-held access token must
 * still be active, must belong to this application, and must belong to the
 * subject of the local session.
 */
export async function authorizeOnlineOperation(
  input: OperationAuthorizationInput,
): Promise<void> {
  if (!input.session) throw new OperationDenied('unauthenticated')

  let result: IntrospectionResult
  try {
    result = await input.introspector.introspect(input.session.tokens.accessToken)
  } catch {
    throw new OperationDenied('introspection_unavailable')
  }

  if (!result.active) throw new OperationDenied('inactive_token')
  if (result.clientId !== input.expectedClientId) throw new OperationDenied('wrong_client')
  if (result.sub !== input.session.profile.subject) throw new OperationDenied('wrong_subject')
  if (result.exp !== undefined && result.exp * 1000 <= input.now) {
    throw new OperationDenied('expired_token')
  }
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
