import { describe, expect, it } from 'vitest'

import {
  ConfigError as PublicConfigError,
  loadPublicConfig,
} from '../src/config.js'
import { ConfigError as ApiConfigError, loadApiConfig } from '../server/config.js'

const publicEnv = {
  VITE_ISSUER: 'https://id.example.com',
  VITE_CLIENT_ID: 'yufu-spa-example',
  VITE_REDIRECT_URI: 'https://spa.example.com/callback',
  VITE_POST_LOGOUT_REDIRECT_URI: 'https://spa.example.com/',
  VITE_SCOPE: 'openid profile email',
  VITE_API_BASE_URL: 'https://api.example.com',
} satisfies Record<string, string>

const apiEnv = {
  ISSUER: 'https://id.example.com',
  API_AUDIENCE: 'urn:app:production',
  SPA_CLIENT_ID: 'yufu-spa-example',
  READER_CLIENT_ID: 'yufu-spa-reader',
  READER_CLIENT_SECRET: 'reader-secret',
  ALLOWED_ORIGIN: 'https://spa.example.com',
  PORT: '3001',
} satisfies Record<string, string>

describe('spa public configuration', () => {
  it('exposes only the approved public parameters', () => {
    const config = loadPublicConfig(publicEnv)

    expect(Object.keys(config).sort()).toEqual([
      'apiBaseUrl',
      'clientId',
      'issuer',
      'postLogoutRedirectUri',
      'redirectUri',
      'scope',
    ])
    expect(config).not.toHaveProperty('clientSecret')
    expect(config).not.toHaveProperty('readerClientSecret')
  })

  it('rejects a secret published through a VITE_ variable', () => {
    expect(() =>
      loadPublicConfig({ ...publicEnv, VITE_CLIENT_SECRET: 'leaked' }),
    ).toThrow(PublicConfigError)
    expect(() =>
      loadPublicConfig({ ...publicEnv, VITE_READER_CLIENT_SECRET: 'leaked' }),
    ).toThrow(/must not be exposed to the browser/)
  })

  it('requires HTTPS outside loopback', () => {
    expect(() =>
      loadPublicConfig({ ...publicEnv, VITE_ISSUER: 'http://id.example.com' }),
    ).toThrow(/HTTPS/)
    expect(
      loadPublicConfig({
        ...publicEnv,
        VITE_ISSUER: 'http://127.0.0.1:8080',
        VITE_REDIRECT_URI: 'http://localhost:5173/callback',
        VITE_POST_LOGOUT_REDIRECT_URI: 'http://localhost:5173/',
        VITE_API_BASE_URL: 'http://127.0.0.1:3001',
      }).issuer,
    ).toBe('http://127.0.0.1:8080')
  })

  it('requires openid in the requested scope', () => {
    expect(() => loadPublicConfig({ ...publicEnv, VITE_SCOPE: 'profile' })).toThrow(
      /openid/,
    )
  })
})

describe('spa protected API configuration', () => {
  it('loads reader credentials only in the API process', () => {
    const config = loadApiConfig(apiEnv)

    expect(config.readerClientId).toBe('yufu-spa-reader')
    expect(config.readerClientSecret).toBe('reader-secret')
    expect(config.audience).toBe('urn:app:production')
    expect(config.clientId).toBe('yufu-spa-example')
    expect(config.allowedOrigin).toBe('https://spa.example.com')
    expect(config.port).toBe(3001)
  })

  it('requires the reader credentials', () => {
    for (const name of [
      'API_AUDIENCE',
      'SPA_CLIENT_ID',
      'READER_CLIENT_ID',
      'READER_CLIENT_SECRET',
    ]) {
      const env: Record<string, string> = { ...apiEnv }
      delete env[name]
      expect(() => loadApiConfig(env), name).toThrow(new RegExp(`${name} is required`))
    }
  })

  it('does not accept reader credentials from public variables', () => {
    const env: Record<string, string> = { ...apiEnv }
    delete env.READER_CLIENT_SECRET
    expect(() =>
      loadApiConfig({ ...env, VITE_READER_CLIENT_SECRET: 'reader-secret' }),
    ).toThrow(ApiConfigError)
  })

  it('enforces HTTPS for the issuer and allowed origin', () => {
    expect(() => loadApiConfig({ ...apiEnv, ISSUER: 'http://id.example.com' })).toThrow(
      /HTTPS/,
    )
    expect(() =>
      loadApiConfig({ ...apiEnv, ALLOWED_ORIGIN: 'http://spa.example.com' }),
    ).toThrow(/HTTPS/)
  })
})
