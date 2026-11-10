import { createHash } from 'node:crypto'

import * as client from 'openid-client'
import type { IDToken } from 'openid-client'

import type { WebConfig } from './config.js'

export class OidcError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'OidcError'
  }
}

/** Minimal local identity derived from a validated ID Token. */
export interface IdentityClaims {
  issuer: string
  subject: string
  displayName: string
  email?: string
  sid?: string
}

/** Tokens stay on the server; the browser only ever receives an opaque id. */
export interface TokenSet {
  accessToken: string
  refreshToken?: string
  idToken?: string
  expiresAt: number
}

export interface AuthorizationRequest {
  url: string
  state: string
  nonce: string
  codeVerifier: string
}

export interface ExchangeInput {
  currentUrl: string
  state: string
  nonce: string
  codeVerifier: string
}

export interface LoginResult {
  claims: IdentityClaims
  tokens: TokenSet
}

export interface OidcClient {
  authorizationRequest(): Promise<AuthorizationRequest>
  exchangeCode(input: ExchangeInput): Promise<LoginResult>
  refresh(refreshToken: string): Promise<TokenSet>
  revoke(token: string): Promise<void>
  /** Standard RP-initiated logout endpoint from discovery metadata. */
  endSessionEndpoint(): Promise<string>
}

export interface OidcClientOptions {
  now?: () => number
  /** Test seam: supply a pre-built configuration instead of discovering one. */
  configuration?: () => Promise<client.Configuration>
}

/** RFC 7636 S256 challenge for a code verifier. */
export function pkceChallenge(codeVerifier: string): string {
  return createHash('sha256').update(codeVerifier).digest('base64url')
}

/**
 * Rejects an error callback before any token request is attempted.
 * Returns the authorization code on success.
 */
export function assertAuthorizationResponse(url: URL): string {
  const error = url.searchParams.get('error')
  if (error) {
    const description = url.searchParams.get('error_description')
    throw new OidcError(
      `authorization failed: ${error}${description ? ` (${description})` : ''}`,
    )
  }
  const code = url.searchParams.get('code')
  if (!code) throw new OidcError('authorization response is missing code')
  return code
}

function stringClaim(claims: IDToken, name: string): string | undefined {
  const value = claims[name]
  return typeof value === 'string' && value.trim() ? value : undefined
}

export function toIdentityClaims(claims: IDToken): IdentityClaims {
  return {
    issuer: claims.iss,
    subject: claims.sub,
    // The login handle is stable; the nickname can be changed by the user.
    displayName: stringClaim(claims, 'preferred_username') ?? stringClaim(claims, 'name') ?? claims.sub,
    email: stringClaim(claims, 'email'),
    sid: stringClaim(claims, 'sid'),
  }
}

function toTokenSet(response: client.TokenEndpointResponse, now: number): TokenSet {
  const expiresIn = typeof response.expires_in === 'number' ? response.expires_in : 300
  return {
    accessToken: response.access_token,
    refreshToken: response.refresh_token,
    idToken: response.id_token,
    expiresAt: now + expiresIn * 1000,
  }
}

export function createOidcClient(
  config: WebConfig,
  options: OidcClientOptions = {},
): OidcClient {
  const now = options.now ?? Date.now
  let pending: Promise<client.Configuration> | undefined

  const load = (): Promise<client.Configuration> => {
    if (options.configuration) return options.configuration()
    pending ??= client.discovery(
      new URL(config.issuer),
      config.clientId,
      { client_secret: config.clientSecret },
      undefined,
      config.allowInsecureLoopback
        ? { execute: [client.allowInsecureRequests] }
        : undefined,
    )
    return pending
  }

  return {
    async authorizationRequest() {
      const state = client.randomState()
      const nonce = client.randomNonce()
      const codeVerifier = client.randomPKCECodeVerifier()
      const url = client.buildAuthorizationUrl(await load(), {
        redirect_uri: config.redirectUri,
        response_type: 'code',
        scope: config.scope,
        state,
        nonce,
        code_challenge: pkceChallenge(codeVerifier),
        code_challenge_method: 'S256',
      })
      return { url: url.href, state, nonce, codeVerifier }
    },

    async exchangeCode(input) {
      const response = await client.authorizationCodeGrant(
        await load(),
        new URL(input.currentUrl),
        {
          pkceCodeVerifier: input.codeVerifier,
          expectedState: input.state,
          expectedNonce: input.nonce,
          idTokenExpected: true,
        },
      )
      const claims = response.claims()
      if (!claims) throw new OidcError('authorization response did not include an ID Token')
      return { claims: toIdentityClaims(claims), tokens: toTokenSet(response, now()) }
    },

    async refresh(refreshToken) {
      const response = await client.refreshTokenGrant(await load(), refreshToken)
      return toTokenSet(response, now())
    },

    async revoke(token) {
      await client.tokenRevocation(await load(), token)
    },

    async endSessionEndpoint() {
      const endpoint = (await load()).serverMetadata().end_session_endpoint
      if (!endpoint) throw new OidcError('issuer does not publish an end_session_endpoint')
      return endpoint
    },
  }
}
