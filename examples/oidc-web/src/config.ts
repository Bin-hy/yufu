/**
 * Server-side configuration for the confidential web example.
 *
 * Every secret stays in this process: the browser only ever sees the issuer,
 * client id and redirect URIs that are part of the authorization request.
 */
export interface WebConfig {
  port: number
  issuer: string
  clientId: string
  clientSecret: string
  redirectUri: string
  postLogoutRedirectUri: string
  scope: string
  sessionSecret: string
  readerClientId: string
  readerClientSecret: string
  allowInsecureLoopback: boolean
}

export class ConfigError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ConfigError'
  }
}

const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '[::1]', '::1'])

function required(env: Record<string, string | undefined>, name: string): string {
  const value = env[name]?.trim()
  if (!value) throw new ConfigError(`${name} is required`)
  return value
}

function absoluteUrl(name: string, value: string): URL {
  let url: URL
  try {
    url = new URL(value)
  } catch {
    throw new ConfigError(`${name} must be an absolute URL`)
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new ConfigError(`${name} must not contain credentials, query or fragment`)
  }
  return url
}

function isLoopback(url: URL): boolean {
  return LOOPBACK_HOSTS.has(url.hostname)
}

function assertTransport(name: string, url: URL, allowInsecureLoopback: boolean): void {
  if (url.protocol === 'https:') return
  if (url.protocol === 'http:' && allowInsecureLoopback && isLoopback(url)) return
  throw new ConfigError(`${name} must use HTTPS (HTTP is only allowed for loopback development)`)
}

export function loadConfig(env: Record<string, string | undefined> = process.env): WebConfig {
  const allowInsecureLoopback = env.ALLOW_INSECURE_LOOPBACK === 'true'
  const issuerUrl = absoluteUrl('ISSUER', required(env, 'ISSUER'))
  assertTransport('ISSUER', issuerUrl, allowInsecureLoopback)

  const redirectUri = absoluteUrl('REDIRECT_URI', required(env, 'REDIRECT_URI'))
  assertTransport('REDIRECT_URI', redirectUri, allowInsecureLoopback)
  const postLogoutRedirectUri = absoluteUrl(
    'POST_LOGOUT_REDIRECT_URI',
    required(env, 'POST_LOGOUT_REDIRECT_URI'),
  )
  assertTransport('POST_LOGOUT_REDIRECT_URI', postLogoutRedirectUri, allowInsecureLoopback)

  const sessionSecret = required(env, 'SESSION_SECRET')
  if (sessionSecret.length < 32) {
    throw new ConfigError('SESSION_SECRET must be at least 32 characters')
  }

  const port = Number(env.PORT ?? '3000')
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new ConfigError('PORT must be an integer between 1 and 65535')
  }

  const scope = (env.SCOPE ?? 'openid profile email')
    .split(/\s+/)
    .filter(Boolean)
  if (!scope.includes('openid')) {
    throw new ConfigError('SCOPE must include openid')
  }

  return {
    port,
    issuer: issuerUrl.toString().replace(/\/$/, ''),
    clientId: required(env, 'CLIENT_ID'),
    clientSecret: required(env, 'CLIENT_SECRET'),
    redirectUri: redirectUri.toString(),
    postLogoutRedirectUri: postLogoutRedirectUri.toString(),
    scope: scope.join(' '),
    sessionSecret,
    readerClientId: required(env, 'READER_CLIENT_ID'),
    readerClientSecret: required(env, 'READER_CLIENT_SECRET'),
    allowInsecureLoopback,
  }
}

/** Browser-visible projection. Never add a secret to this type. */
export interface PublicConfig {
  issuer: string
  clientId: string
  redirectUri: string
  postLogoutRedirectUri: string
  scope: string
}

export function publicConfig(config: WebConfig): PublicConfig {
  return {
    issuer: config.issuer,
    clientId: config.clientId,
    redirectUri: config.redirectUri,
    postLogoutRedirectUri: config.postLogoutRedirectUri,
    scope: config.scope,
  }
}
