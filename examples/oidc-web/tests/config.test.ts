import { describe, expect, it } from 'vitest'

import { ConfigError, loadConfig, publicConfig } from '../src/config.js'

const baseEnv = {
  ISSUER: 'https://id.example.com',
  CLIENT_ID: 'yufu-web-example',
  CLIENT_SECRET: 'web-client-secret',
  REDIRECT_URI: 'https://app.example.com/callback',
  POST_LOGOUT_REDIRECT_URI: 'https://app.example.com/',
  SCOPE: 'openid profile email',
  SESSION_SECRET: 'x'.repeat(32),
  READER_CLIENT_ID: 'yufu-web-reader',
  READER_CLIENT_SECRET: 'reader-secret',
  PORT: '3000',
} satisfies Record<string, string>

describe('web example configuration', () => {
  it('loads a complete HTTPS configuration', () => {
    const config = loadConfig(baseEnv)

    expect(config.issuer).toBe('https://id.example.com')
    expect(config.port).toBe(3000)
    expect(config.scope).toBe('openid profile email')
    expect(config.allowInsecureLoopback).toBe(false)
  })

  it('never exposes secrets through the browser projection', () => {
    const publicView = publicConfig(loadConfig(baseEnv))

    expect(Object.keys(publicView).sort()).toEqual([
      'clientId',
      'issuer',
      'postLogoutRedirectUri',
      'redirectUri',
      'scope',
    ])
    const serialized = JSON.stringify(publicView)
    expect(serialized).not.toContain(baseEnv.CLIENT_SECRET)
    expect(serialized).not.toContain(baseEnv.READER_CLIENT_SECRET)
    expect(serialized).not.toContain(baseEnv.SESSION_SECRET)
  })

  it('rejects plaintext HTTP for a non-loopback issuer', () => {
    expect(() =>
      loadConfig({ ...baseEnv, ISSUER: 'http://id.example.com' }),
    ).toThrow(ConfigError)

    expect(() =>
      loadConfig({
        ...baseEnv,
        ISSUER: 'http://id.example.com',
        ALLOW_INSECURE_LOOPBACK: 'true',
      }),
    ).toThrow(/HTTPS/)
  })

  it('allows plaintext HTTP only for explicit loopback development', () => {
    const config = loadConfig({
      ...baseEnv,
      ISSUER: 'http://127.0.0.1:8080',
      REDIRECT_URI: 'http://localhost:3000/callback',
      POST_LOGOUT_REDIRECT_URI: 'http://localhost:3000/',
      ALLOW_INSECURE_LOOPBACK: 'true',
    })

    expect(config.issuer).toBe('http://127.0.0.1:8080')
    expect(config.allowInsecureLoopback).toBe(true)
  })

  it('reports every missing required value', () => {
    for (const name of [
      'ISSUER',
      'CLIENT_ID',
      'CLIENT_SECRET',
      'REDIRECT_URI',
      'POST_LOGOUT_REDIRECT_URI',
      'SESSION_SECRET',
      'READER_CLIENT_ID',
      'READER_CLIENT_SECRET',
    ]) {
      const env: Record<string, string> = { ...baseEnv }
      delete env[name]
      expect(() => loadConfig(env), name).toThrow(new RegExp(`${name} is required`))
    }
  })

  it('requires a session secret of at least 32 characters', () => {
    expect(() => loadConfig({ ...baseEnv, SESSION_SECRET: 'short' })).toThrow(
      /at least 32 characters/,
    )
  })

  it('requires openid in the requested scope', () => {
    expect(() => loadConfig({ ...baseEnv, SCOPE: 'profile email' })).toThrow(/openid/)
  })

  it('rejects URLs carrying credentials, query or fragment', () => {
    expect(() =>
      loadConfig({ ...baseEnv, REDIRECT_URI: 'https://user:pass@app.example.com/callback' }),
    ).toThrow(/credentials/)
    expect(() =>
      loadConfig({ ...baseEnv, POST_LOGOUT_REDIRECT_URI: 'https://app.example.com/?next=1' }),
    ).toThrow(/query or fragment/)
  })

  it('validates the listening port', () => {
    expect(() => loadConfig({ ...baseEnv, PORT: '0' })).toThrow(/PORT/)
    expect(() => loadConfig({ ...baseEnv, PORT: 'not-a-port' })).toThrow(/PORT/)
  })
})
