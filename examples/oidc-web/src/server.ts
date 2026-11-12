import { pathToFileURL } from 'node:url'

import { parse as parseCookie } from 'cookie'
import express, { type Request, type Response } from 'express'

import { loadConfig, type WebConfig } from './config.js'
import {
  applyBackchannelLogout,
  buildEndSessionUrl,
  createLogoutTokenVerifier,
  LogoutReplayGuard,
  newLogoutState,
  type VerifiedLogoutToken,
} from './logout.js'
import {
  assertAuthorizationResponse,
  createOidcClient,
  OidcError,
  type OidcClient,
  parseAuthorizationOptions,
  toProtocolEvidence,
} from './oidc.js'
import {
  AuthorizationCache,
  authorizeOnlineOperation,
  createIntrospector,
  csrfToken,
  OperationDenied,
  originAllowed,
  StaleAuthorization,
  verifyCsrfToken,
  type Introspector,
} from './protection.js'
import {
  clearCookie,
  PENDING_COOKIE,
  PendingStore,
  pendingCookie,
  ProfileDirectory,
  safeReturnTo,
  SESSION_COOKIE,
  sessionCookie,
  SessionStore,
  type SessionRecord,
} from './session.js'

export interface WebAppDependencies {
  config: WebConfig
  oidc: OidcClient
  sessions: SessionStore
  profiles: ProfileDirectory
  pending: PendingStore
  introspector: Introspector
  cache?: AuthorizationCache
  verifyLogoutToken: (token: string) => Promise<VerifiedLogoutToken>
  replayGuard: LogoutReplayGuard
  now?: () => number
}

function escapeHtml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')
}

function cookiesOf(request: Request): Record<string, string | undefined> {
  const header = request.headers.cookie
  return header ? parseCookie(header) : {}
}

function evidenceText(value: string | number | undefined): string {
  return value === undefined || value === '' ? '—' : String(value)
}

function renderHome(session: SessionRecord | undefined, csrf: string): string {
  const body = session
    ? `<h1>Signed in</h1>
       <p data-testid="display-name">${escapeHtml(session.profile.displayName)}</p>
       <p data-testid="local-subject">${escapeHtml(session.profile.subject)}</p>
       <section aria-label="Verified OIDC evidence">
         <p data-testid="issuer">${escapeHtml(session.protocol.issuer)}</p>
         <p data-testid="client-id">${escapeHtml(session.protocol.clientId)}</p>
         <p data-testid="audience">${escapeHtml(session.protocol.audience.join(', '))}</p>
         <p data-testid="sid">${escapeHtml(evidenceText(session.protocol.sid))}</p>
         <p data-testid="auth-time">${escapeHtml(evidenceText(session.protocol.authTime))}</p>
         <p data-testid="state-validated">${session.protocol.stateVerified}</p>
         <p data-testid="nonce-validated">${session.protocol.nonceVerified}</p>
         <p data-testid="access-token-fingerprint">${escapeHtml(session.tokens.accessTokenFingerprint)}</p>
       </section>
       <form method="post" action="/api/operation">
         <input type="hidden" name="_csrf" value="${escapeHtml(csrf)}">
         <button type="submit">Run protected operation</button>
       </form>
       <form method="post" action="/logout">
         <input type="hidden" name="_csrf" value="${escapeHtml(csrf)}">
         <button type="submit">Sign out of this app</button>
       </form>
       <p><a href="/logout/central">Sign out of the identity center</a></p>`
    : `<h1>Not signed in</h1>
       <p><a href="/login">Sign in with Cobotrix</a></p>`
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>oidc-web example</title></head><body>${body}</body></html>`
}

export function createApp(deps: WebAppDependencies): express.Express {
  const { config, oidc, sessions, profiles, pending, introspector, replayGuard } = deps
  const cache = deps.cache ?? new AuthorizationCache()
  const now = deps.now ?? Date.now
  const secure = config.issuer.startsWith('https:')

  const currentSession = (request: Request): SessionRecord | undefined => {
    const id = cookiesOf(request)[SESSION_COOKIE]
    return id ? sessions.get(id) : undefined
  }

  const sameOrigin = (request: Request): boolean =>
    originAllowed(request.headers.origin, config.redirectUri)

  const app = express()
  app.disable('x-powered-by')
  app.use(express.urlencoded({ extended: false }))

  app.get('/', (request, response) => {
    const session = currentSession(request)
    const csrf = session ? csrfToken(session.id, config.sessionSecret) : ''
    response.type('html').send(renderHome(session, csrf))
  })

  app.get('/login', async (request, response) => {
    const prompt = request.query.prompt
    const maxAge = request.query.max_age
    if (
      (prompt !== undefined && typeof prompt !== 'string') ||
      (maxAge !== undefined && typeof maxAge !== 'string')
    ) {
      response.status(400).type('html').send('<h1>Invalid login options</h1>')
      return
    }
    const options = parseAuthorizationOptions(prompt, maxAge)
    const authorization = await oidc.authorizationRequest(options)
    pending.put({
      state: authorization.state,
      nonce: authorization.nonce,
      codeVerifier: authorization.codeVerifier,
      returnTo: safeReturnTo(request.query.return_to as string | undefined),
    })
    response.setHeader('Set-Cookie', pendingCookie(authorization.state, secure))
    response.redirect(302, authorization.url)
  })

  app.get('/callback', async (request, response) => {
    const currentUrl = new URL(request.originalUrl, config.redirectUri)
    const state = currentUrl.searchParams.get('state') ?? ''
    const bound = cookiesOf(request)[PENDING_COOKIE] === state
    const record = bound ? pending.consume(state) : undefined
    if (!record) {
      response.status(400).type('html').send('<h1>Invalid authorization response</h1>')
      return
    }

    assertAuthorizationResponse(currentUrl)
    const result = await oidc.exchangeCode({
      currentUrl: currentUrl.href,
      state: record.state,
      nonce: record.nonce,
      codeVerifier: record.codeVerifier,
    })

    const profile = profiles.upsert(result.claims, now())
    const session = sessions.create(
      profile,
      result.tokens,
      toProtocolEvidence(result.claims, config.clientId),
    )
    response.setHeader('Set-Cookie', [
      sessionCookie(session.id, secure),
      clearCookie(PENDING_COOKIE, secure),
    ])
    response.redirect(302, record.returnTo)
  })

  app.get('/api/me', (request, response) => {
    const session = currentSession(request)
    if (!session) {
      response.status(401).json({ error: 'unauthenticated' })
      return
    }
    response.json({
      issuer: session.profile.issuer,
      subject: session.profile.subject,
      displayName: session.profile.displayName,
      email: session.profile.email,
      protocol: session.protocol,
      accessTokenFingerprint: session.tokens.accessTokenFingerprint,
    })
  })

  // Local logout: clears only this application's session, then best-effort revokes
  // this application's grant. Other relying parties keep their own sessions.
  app.post('/logout', async (request, response) => {
    const session = currentSession(request)
    if (!session) {
      response.redirect(302, '/')
      return
    }
    const body = request.body as Record<string, unknown> | undefined
    const csrf = typeof body?._csrf === 'string' ? body._csrf : undefined
    if (!sameOrigin(request) || !verifyCsrfToken(session.id, config.sessionSecret, csrf)) {
      response.status(403).json({ error: 'csrf_rejected' })
      return
    }

    sessions.delete(session.id)
    try {
      await oidc.revoke(session.tokens.accessToken)
    } catch {
      // Revocation is best effort; the local session is already gone.
    }
    response.setHeader('Set-Cookie', clearCookie(SESSION_COOKIE, secure))
    response.redirect(302, '/')
  })

  // Central logout: standard RP-initiated logout at the discovered endpoint.
  app.get('/logout/central', async (request, response) => {
    const session = currentSession(request)
    if (!session) {
      response.redirect(302, '/')
      return
    }
    const endpoint = await oidc.endSessionEndpoint()
    const url = buildEndSessionUrl({
      endSessionEndpoint: endpoint,
      idTokenHint: session.tokens.idToken,
      postLogoutRedirectUri: config.postLogoutRedirectUri,
      state: newLogoutState(),
    })
    sessions.delete(session.id)
    response.setHeader('Set-Cookie', clearCookie(SESSION_COOKIE, secure))
    response.redirect(302, url)
  })

  // Back-channel logout receiver: verify, then clear by sid (or sub) idempotently.
  app.post('/oidc/backchannel-logout', async (request, response) => {
    const body = request.body as Record<string, unknown> | undefined
    const token = typeof body?.logout_token === 'string' ? body.logout_token : ''
    try {
      const verified = await deps.verifyLogoutToken(token)
      if (replayGuard.seen(verified.jti)) {
        response.status(200).json({ status: 'duplicate' })
        return
      }
      const removed = applyBackchannelLogout(sessions, verified)
      const cached = cache.clearForLogout(verified.sid, verified.subject)
      response.status(200).json({ status: 'ok', removed, cached })
    } catch {
      response.status(400).json({ error: 'invalid_logout_token' })
    }
  })

  // Protected write: local CSRF/origin checks, then mandatory online introspection.
  app.post('/api/operation', async (request, response) => {
    const session = currentSession(request)
    if (!session) {
      response.status(401).json({ error: 'unauthenticated' })
      return
    }
    const body = request.body as Record<string, unknown> | undefined
    const csrf = typeof body?._csrf === 'string' ? body._csrf : undefined
    if (!sameOrigin(request) || !verifyCsrfToken(session.id, config.sessionSecret, csrf)) {
      response.status(403).json({ error: 'csrf_rejected' })
      return
    }

    try {
      await authorizeOnlineOperation({
        session,
        introspector,
        expectedClientId: config.clientId,
        now: now(),
        cache,
      })
    } catch (error) {
      if (error instanceof OperationDenied) {
        response.status(403).json({ error: error.reason })
        return
      }
      if (error instanceof StaleAuthorization) {
        response.status(403).json({ error: 'stale_authorization' })
        return
      }
      throw error
    }

    response.json({ ok: true, operation: 'example-write' })
  })

  app.use((error: unknown, _request: Request, response: Response, _next: unknown) => {
    if (error instanceof OidcError) {
      response
        .status(400)
        .type('html')
        .send(`<h1>Sign-in failed</h1><p>${escapeHtml(error.message)}</p>`)
      return
    }
    response.status(500).json({ error: 'internal_error' })
  })

  return app
}

function main(): void {
  const config = loadConfig()
  const app = createApp({
    config,
    oidc: createOidcClient(config),
    sessions: new SessionStore(),
    profiles: new ProfileDirectory(),
    pending: new PendingStore(),
    introspector: createIntrospector(config),
    cache: new AuthorizationCache(),
    verifyLogoutToken: createLogoutTokenVerifier({
      issuer: config.issuer,
      clientId: config.clientId,
    }),
    replayGuard: new LogoutReplayGuard(),
  })
  app.listen(config.port, () => {
    process.stdout.write(`oidc-web example listening on http://127.0.0.1:${config.port}\n`)
  })
}

const entry = process.argv[1]
if (entry && import.meta.url === pathToFileURL(entry).href) {
  main()
}
