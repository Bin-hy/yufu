/**
 * Configuration for the companion protected API process.
 *
 * The reader credentials live only here. They are never imported by `src/`,
 * so they cannot end up in the browser bundle.
 */
export interface ApiConfig {
  port: number
  issuer: string
  /** The application's registered access audience (for example `urn:app:production`). */
  audience: string
  /** The SPA's OAuth client id, used to reject tokens minted for another client. */
  clientId: string
  readerClientId: string
  readerClientSecret: string
  allowedOrigin: string
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

function assertTransport(name: string, url: URL, allowInsecureLoopback: boolean): void {
  if (url.protocol === 'https:') return
  if (url.protocol === 'http:' && allowInsecureLoopback && LOOPBACK_HOSTS.has(url.hostname)) return
  throw new ConfigError(`${name} must use HTTPS (HTTP is only allowed for loopback development)`)
}

export function loadApiConfig(
  env: Record<string, string | undefined> = process.env,
): ApiConfig {
  const allowInsecureLoopback = env.ALLOW_INSECURE_LOOPBACK === 'true'

  const issuerUrl = absoluteUrl('ISSUER', required(env, 'ISSUER'))
  assertTransport('ISSUER', issuerUrl, allowInsecureLoopback)

  const allowedOriginUrl = absoluteUrl('ALLOWED_ORIGIN', required(env, 'ALLOWED_ORIGIN'))
  assertTransport('ALLOWED_ORIGIN', allowedOriginUrl, allowInsecureLoopback)

  const port = Number(env.PORT ?? '3001')
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new ConfigError('PORT must be an integer between 1 and 65535')
  }

  return {
    port,
    issuer: issuerUrl.toString().replace(/\/$/, ''),
    audience: required(env, 'API_AUDIENCE'),
    clientId: required(env, 'SPA_CLIENT_ID'),
    readerClientId: required(env, 'READER_CLIENT_ID'),
    readerClientSecret: required(env, 'READER_CLIENT_SECRET'),
    allowedOrigin: allowedOriginUrl.origin,
    allowInsecureLoopback,
  }
}
