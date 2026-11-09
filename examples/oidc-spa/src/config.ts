/**
 * Browser-visible configuration for the public SPA example.
 *
 * A public client has no secret. Only the approved public parameters below may
 * be read from the environment; anything secret-looking is rejected outright so
 * a misconfigured deployment fails loudly instead of leaking into the bundle.
 */
export interface SpaConfig {
  issuer: string
  clientId: string
  redirectUri: string
  postLogoutRedirectUri: string
  scope: string
  apiBaseUrl: string
}

export class ConfigError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ConfigError'
  }
}

const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '[::1]', '::1'])

const FORBIDDEN_PUBLIC_KEYS = [
  'VITE_CLIENT_SECRET',
  'VITE_READER_CLIENT_ID',
  'VITE_READER_CLIENT_SECRET',
  'VITE_SESSION_SECRET',
]

export type PublicEnv = Record<string, string | undefined>

function required(env: PublicEnv, name: string): string {
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

function assertTransport(name: string, url: URL): void {
  if (url.protocol === 'https:') return
  if (url.protocol === 'http:' && LOOPBACK_HOSTS.has(url.hostname)) return
  throw new ConfigError(`${name} must use HTTPS (HTTP is only allowed for loopback development)`)
}

/** Rejects any attempt to publish a secret through VITE_* variables. */
export function assertNoSecretsInPublicEnv(env: PublicEnv): void {
  for (const key of FORBIDDEN_PUBLIC_KEYS) {
    if (env[key]?.trim()) {
      throw new ConfigError(`${key} must not be exposed to the browser`)
    }
  }
}

export function loadPublicConfig(env: PublicEnv): SpaConfig {
  assertNoSecretsInPublicEnv(env)

  const issuerUrl = absoluteUrl('VITE_ISSUER', required(env, 'VITE_ISSUER'))
  assertTransport('VITE_ISSUER', issuerUrl)

  const redirectUri = absoluteUrl('VITE_REDIRECT_URI', required(env, 'VITE_REDIRECT_URI'))
  assertTransport('VITE_REDIRECT_URI', redirectUri)

  const postLogoutRedirectUri = absoluteUrl(
    'VITE_POST_LOGOUT_REDIRECT_URI',
    required(env, 'VITE_POST_LOGOUT_REDIRECT_URI'),
  )
  assertTransport('VITE_POST_LOGOUT_REDIRECT_URI', postLogoutRedirectUri)

  const apiBaseUrl = absoluteUrl('VITE_API_BASE_URL', required(env, 'VITE_API_BASE_URL'))
  assertTransport('VITE_API_BASE_URL', apiBaseUrl)

  const scope = (env.VITE_SCOPE ?? 'openid profile email').split(/\s+/).filter(Boolean)
  if (!scope.includes('openid')) {
    throw new ConfigError('VITE_SCOPE must include openid')
  }

  return {
    issuer: issuerUrl.toString().replace(/\/$/, ''),
    clientId: required(env, 'VITE_CLIENT_ID'),
    redirectUri: redirectUri.toString(),
    postLogoutRedirectUri: postLogoutRedirectUri.toString(),
    scope: scope.join(' '),
    apiBaseUrl: apiBaseUrl.toString().replace(/\/$/, ''),
  }
}

/** Reads the build-time public configuration injected by Vite. */
export function browserConfig(): SpaConfig {
  return loadPublicConfig(import.meta.env as PublicEnv)
}
