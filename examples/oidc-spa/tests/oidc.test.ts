import { describe, expect, it, vi } from 'vitest'

import { ApiError, createApiClient } from '../src/api.js'
import type { SpaConfig } from '../src/config.js'
import {
  createSpaOidc,
  toSpaSession,
  userManagerSettings,
  type OidcUserLike,
  type UserManagerLike,
} from '../src/oidc.js'

const config: SpaConfig = {
  issuer: 'https://id.example.com',
  clientId: 'cobotrix-spa-example',
  redirectUri: 'https://spa.example.com/callback',
  postLogoutRedirectUri: 'https://spa.example.com/',
  scope: 'openid profile email',
  apiBaseUrl: 'https://api.example.com',
}

class MemoryStorage implements Storage {
  #items = new Map<string, string>()

  get length(): number {
    return this.#items.size
  }

  clear(): void {
    this.#items.clear()
  }

  getItem(key: string): string | null {
    return this.#items.get(key) ?? null
  }

  key(index: number): string | null {
    return [...this.#items.keys()][index] ?? null
  }

  removeItem(key: string): void {
    this.#items.delete(key)
  }

  setItem(key: string, value: string): void {
    this.#items.set(key, value)
  }
}

function user(overrides: Partial<OidcUserLike> = {}): OidcUserLike {
  return {
    access_token: 'access-1',
    id_token: 'id-1',
    refresh_token: 'refresh-1',
    expires_at: 2_000,
    profile: {
      iss: config.issuer,
      sub: 'user-1',
      name: 'Alice',
      email: 'alice@example.com',
    },
    ...overrides,
  }
}

function manager(overrides: Partial<UserManagerLike> = {}): UserManagerLike {
  return {
    signinRedirect: vi.fn(async () => undefined),
    signinRedirectCallback: vi.fn(async () => user()),
    signinSilent: vi.fn(async () => user()),
    signoutRedirect: vi.fn(async () => undefined),
    removeUser: vi.fn(async () => undefined),
    getUser: vi.fn(async () => user()),
    ...overrides,
  }
}

describe('public client settings', () => {
  it('uses code flow with PKCE and never carries a client secret', () => {
    const settings = userManagerSettings(config, new MemoryStorage() as unknown as Storage)

    expect(settings.response_type).toBe('code')
    expect(settings.disablePKCE).toBeUndefined()
    expect(settings.client_secret).toBeUndefined()
    expect(JSON.stringify(settings)).not.toContain('secret')
  })

  it('disables iframe-based silent recovery', () => {
    const settings = userManagerSettings(config, new MemoryStorage() as unknown as Storage)

    expect(settings.automaticSilentRenew).toBe(false)
    expect(settings.monitorSession).toBe(false)
    expect(settings.silent_redirect_uri).toBeUndefined()
  })

  it('keeps state and user records in the tab-scoped storage', async () => {
    const storage = new MemoryStorage() as unknown as Storage
    const settings = userManagerSettings(config, storage)

    await settings.stateStore?.set('state-probe', JSON.stringify({ value: 1 }))
    await settings.userStore?.set('user-probe', JSON.stringify({ value: 2 }))

    expect(storage.getItem('oidc.state-probe')).toContain('"value":1')
    expect(storage.getItem('oidc.user-probe')).toContain('"value":2')
  })
})

describe('local session projection', () => {
  it('exposes only identity, access token and expiry', () => {
    const session = toSpaSession(user(), 1_000)

    expect(Object.keys(session).sort()).toEqual(['accessToken', 'expiresAt', 'identity'])
    expect(session).not.toHaveProperty('refreshToken')
    expect(session).not.toHaveProperty('idToken')
    expect(session.identity).toEqual({
      issuer: config.issuer,
      subject: 'user-1',
      displayName: 'Alice',
      email: 'alice@example.com',
    })
    expect(session.expiresAt).toBe(2_000_000)
  })

  it('rejects a user record without iss or sub', () => {
    expect(() => toSpaSession(user({ profile: { sub: 'user-1' } }), 1_000)).toThrow(/iss/)
    expect(() => toSpaSession(user({ profile: { iss: config.issuer } }), 1_000)).toThrow(/sub/)
  })
})

describe('spa sign-in flow', () => {
  it('starts a top-level redirect', async () => {
    const fake = manager()
    const oidc = createSpaOidc(config, { storage: new MemoryStorage() as unknown as Storage, manager: fake })

    await oidc.signIn()

    expect(fake.signinRedirect).toHaveBeenCalledTimes(1)
  })

  it('completes the callback into a local session', async () => {
    const oidc = createSpaOidc(config, {
      storage: new MemoryStorage() as unknown as Storage,
      manager: manager(),
      now: () => 1_000,
    })

    const session = await oidc.completeSignIn('https://spa.example.com/callback?code=c&state=s')

    expect(session.identity.subject).toBe('user-1')
    expect(session.accessToken).toBe('access-1')
  })

  it('returns no session when the tab has no user', async () => {
    const oidc = createSpaOidc(config, {
      storage: new MemoryStorage() as unknown as Storage,
      manager: manager({ getUser: async () => null }),
    })

    expect(await oidc.currentSession()).toBeUndefined()
  })
})

describe('per-tab refresh', () => {
  it('shares one refresh across concurrent callers', async () => {
    let release: (() => void) | undefined
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    const signinSilent = vi.fn(async () => {
      await gate
      return user({ access_token: 'access-2', expires_at: 9_000 })
    })
    const oidc = createSpaOidc(config, {
      storage: new MemoryStorage() as unknown as Storage,
      manager: manager({ signinSilent }),
      now: () => 1_000,
    })

    const first = oidc.refresh()
    const second = oidc.refresh()
    release?.()
    const [a, b] = await Promise.all([first, second])

    expect(signinSilent).toHaveBeenCalledTimes(1)
    expect(a.accessToken).toBe('access-2')
    expect(b.accessToken).toBe('access-2')
  })

  it('refreshes an expired access token before an API call', async () => {
    const signinSilent = vi.fn(async () => user({ access_token: 'access-2', expires_at: 9_000 }))
    const oidc = createSpaOidc(config, {
      storage: new MemoryStorage() as unknown as Storage,
      manager: manager({ signinSilent }),
      now: () => 5_000_000,
    })

    expect(await oidc.accessToken()).toBe('access-2')
    expect(signinSilent).toHaveBeenCalledTimes(1)
  })

  it('fails closed when no refresh token is available', async () => {
    const oidc = createSpaOidc(config, {
      storage: new MemoryStorage() as unknown as Storage,
      manager: manager({
        signinSilent: async () => {
          throw new Error('No silent_redirect_uri configured')
        },
      }),
    })

    await expect(oidc.refresh()).rejects.toThrow(/silent_redirect_uri/)
  })
})

describe('logout separation', () => {
  it('clears only the tab on local logout', async () => {
    const fake = manager()
    const oidc = createSpaOidc(config, { storage: new MemoryStorage() as unknown as Storage, manager: fake })

    await oidc.signOutLocal()

    expect(fake.removeUser).toHaveBeenCalledTimes(1)
    expect(fake.signoutRedirect).not.toHaveBeenCalled()
  })

  it('leaves for the center on central logout', async () => {
    const fake = manager()
    const oidc = createSpaOidc(config, { storage: new MemoryStorage() as unknown as Storage, manager: fake })

    await oidc.signOutCentral()

    expect(fake.signoutRedirect).toHaveBeenCalledTimes(1)
    expect(fake.removeUser).not.toHaveBeenCalled()
  })
})

describe('companion api client', () => {
  it('sends the access token only as a bearer header', async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const client = createApiClient(config, {
      fetch: (async (url: string | URL | Request, init?: RequestInit) => {
        calls.push({ url: String(url), init: init ?? {} })
        return new Response(JSON.stringify({ ok: true, operation: 'example-write' }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }) as typeof fetch,
    })

    const result = await client.runOperation('access-1')

    expect(result).toEqual({ ok: true, operation: 'example-write' })
    expect(calls[0]?.url).toBe('https://api.example.com/api/operation')
    const headers = calls[0]?.init.headers as Record<string, string>
    expect(headers.authorization).toBe('Bearer access-1')
    expect(JSON.stringify(calls[0]?.init)).not.toContain('refresh-1')
  })

  it('surfaces the denial reason', async () => {
    const client = createApiClient(config, {
      fetch: (async () =>
        new Response(JSON.stringify({ error: 'inactive_token' }), {
          status: 403,
          headers: { 'content-type': 'application/json' },
        })) as typeof fetch,
    })

    await expect(client.runOperation('access-1')).rejects.toBeInstanceOf(ApiError)
    await expect(client.runOperation('access-1')).rejects.toMatchObject({
      status: 403,
      reason: 'inactive_token',
    })
  })
})
