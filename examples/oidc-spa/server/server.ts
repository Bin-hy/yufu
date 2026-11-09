import { pathToFileURL } from 'node:url'

import express, { type NextFunction, type Request, type Response } from 'express'

import { loadApiConfig, type ApiConfig } from './config.js'
import {
  clearAuthorizationCache,
  createLogoutTokenVerifier,
  LogoutReplayGuard,
  type VerifiedLogoutToken,
} from './logout.js'
import {
  AccessTokenError,
  AuthorizationCache,
  authorizeProtectedOperation,
  createAccessTokenVerifier,
  createIntrospector,
  OperationDenied,
  originAllowed,
  StaleAuthorization,
  type AccessTokenClaims,
  type Introspector,
} from './protection.js'

export interface ApiDependencies {
  config: ApiConfig
  verifyAccessToken: (token: string) => Promise<AccessTokenClaims>
  introspector: Introspector
  cache: AuthorizationCache
  verifyLogoutToken: (token: string) => Promise<VerifiedLogoutToken>
  replayGuard: LogoutReplayGuard
}

function bearerToken(request: Request): string {
  const header = request.headers.authorization ?? ''
  if (!header.startsWith('Bearer ')) return ''
  return header.slice('Bearer '.length).trim()
}

export function createApi(deps: ApiDependencies): express.Express {
  const { config, introspector, cache, replayGuard } = deps
  const app = express()
  app.disable('x-powered-by')
  app.use(express.urlencoded({ extended: false }))
  app.use(express.json({ limit: '16kb' }))

  // Browser calls are accepted only from the registered SPA origin.
  app.use((request: Request, response: Response, next: NextFunction) => {
    const origin = request.headers.origin
    if (origin) {
      if (!originAllowed(origin, config.allowedOrigin)) {
        response.status(403).json({ error: 'origin_rejected' })
        return
      }
      response.setHeader('Access-Control-Allow-Origin', origin)
      response.setHeader('Vary', 'Origin')
      response.setHeader('Access-Control-Allow-Headers', 'authorization, content-type')
      response.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS')
    }
    if (request.method === 'OPTIONS') {
      response.status(204).end()
      return
    }
    next()
  })

  app.get('/healthz', (_request, response) => {
    response.json({ status: 'ok' })
  })

  // Protected write. The API never exchanges authorization codes for the SPA.
  app.post('/api/operation', async (request, response) => {
    const token = bearerToken(request)
    if (!token) {
      response.status(401).json({ error: 'missing_bearer_token' })
      return
    }

    let claims: AccessTokenClaims
    try {
      claims = await deps.verifyAccessToken(token)
    } catch (error) {
      if (error instanceof AccessTokenError) {
        response.status(401).json({ error: 'invalid_token' })
        return
      }
      throw error
    }

    try {
      const entry = await authorizeProtectedOperation({
        token,
        claims,
        cache,
        introspector,
        expectedClientId: config.clientId,
      })
      // Re-check immediately before the side effect.
      cache.assertFresh(entry)
      response.json({ ok: true, operation: 'example-write', subject: entry.subject })
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
  })

  // Back-channel logout: verify, clear cached authorizations, stay idempotent.
  app.post('/oidc/backchannel-logout', async (request, response) => {
    const body = request.body as Record<string, unknown> | undefined
    const token = typeof body?.logout_token === 'string' ? body.logout_token : ''
    try {
      const verified = await deps.verifyLogoutToken(token)
      if (replayGuard.seen(verified.jti)) {
        response.status(200).json({ status: 'duplicate' })
        return
      }
      const removed = clearAuthorizationCache(cache, verified)
      response.status(200).json({ status: 'ok', removed })
    } catch {
      response.status(400).json({ error: 'invalid_logout_token' })
    }
  })

  return app
}

function main(): void {
  const config = loadApiConfig()
  const app = createApi({
    config,
    verifyAccessToken: createAccessTokenVerifier({
      issuer: config.issuer,
      audience: config.audience,
    }),
    introspector: createIntrospector(config),
    cache: new AuthorizationCache(),
    verifyLogoutToken: createLogoutTokenVerifier({
      issuer: config.issuer,
      clientId: config.clientId,
    }),
    replayGuard: new LogoutReplayGuard(),
  })
  app.listen(config.port, () => {
    process.stdout.write(`oidc-spa api listening on http://127.0.0.1:${config.port}\n`)
  })
}

const entry = process.argv[1]
if (entry && import.meta.url === pathToFileURL(entry).href) {
  main()
}
