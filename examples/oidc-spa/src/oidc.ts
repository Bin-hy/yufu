import { UserManager, WebStorageStateStore, type UserManagerSettings } from 'oidc-client-ts'

import type { SpaConfig } from './config.js'

/** Minimal local identity derived from the validated ID Token. */
export interface SpaIdentity {
  issuer: string
  subject: string
  displayName: string
  email?: string
}

/** What the page may hold in memory. Refresh secrets stay in tab storage. */
export interface SpaSession {
  identity: SpaIdentity
  accessToken: string
  expiresAt: number
}

export interface OidcUserLike {
  access_token: string
  id_token?: string
  refresh_token?: string
  expires_at?: number
  profile: Record<string, unknown>
}

export interface UserManagerLike {
  signinRedirect(): Promise<void>
  signinRedirectCallback(url?: string): Promise<OidcUserLike>
  signinSilent(): Promise<OidcUserLike | null>
  signoutRedirect(): Promise<void>
  removeUser(): Promise<void>
  getUser(): Promise<OidcUserLike | null>
}

export interface SpaOidc {
  signIn(): Promise<void>
  completeSignIn(url?: string): Promise<SpaSession>
  currentSession(): Promise<SpaSession | undefined>
  refresh(): Promise<SpaSession>
  accessToken(): Promise<string>
  signOutLocal(): Promise<void>
  signOutCentral(): Promise<void>
}

export interface SpaOidcOptions {
  storage?: Storage
  manager?: UserManagerLike
  now?: () => number
}

/**
 * Public-client settings.
 *
 * - `response_type: code` with PKCE S256 is oidc-client-ts's default.
 * - `automaticSilentRenew` and `monitorSession` are off, and no
 *   `silent_redirect_uri` is configured, so recovery can only use the refresh
 *   token through a top-level flow, never a hidden iframe.
 * - Both stores are bound to the caller's storage; `main.ts` passes
 *   `sessionStorage`, so each tab keeps its own state and refresh token.
 */
export function userManagerSettings(
  config: SpaConfig,
  storage: Storage,
): UserManagerSettings {
  const stateStore = new WebStorageStateStore({ store: storage })
  return {
    authority: config.issuer,
    client_id: config.clientId,
    redirect_uri: config.redirectUri,
    post_logout_redirect_uri: config.postLogoutRedirectUri,
    response_type: 'code',
    scope: config.scope,
    automaticSilentRenew: false,
    monitorSession: false,
    loadUserInfo: false,
    userStore: stateStore,
    stateStore,
  }
}

function stringClaim(profile: Record<string, unknown>, name: string): string | undefined {
  const value = profile[name]
  return typeof value === 'string' && value.trim() ? value : undefined
}

export function toSpaSession(user: OidcUserLike, now: number): SpaSession {
  const issuer = stringClaim(user.profile, 'iss')
  const subject = stringClaim(user.profile, 'sub')
  if (!issuer || !subject) throw new Error('ID Token is missing iss or sub')

  return {
    identity: {
      issuer,
      subject,
      displayName:
        stringClaim(user.profile, 'name') ??
        stringClaim(user.profile, 'preferred_username') ??
        subject,
      email: stringClaim(user.profile, 'email'),
    },
    accessToken: user.access_token,
    expiresAt: user.expires_at ? user.expires_at * 1000 : now,
  }
}

function expired(user: OidcUserLike, now: number): boolean {
  return user.expires_at !== undefined && user.expires_at * 1000 <= now
}

export function createSpaOidc(config: SpaConfig, options: SpaOidcOptions = {}): SpaOidc {
  const now = options.now ?? Date.now
  const storage = options.storage ?? window.sessionStorage
  const manager =
    options.manager ??
    (new UserManager(userManagerSettings(config, storage)) as unknown as UserManagerLike)

  let inflight: Promise<SpaSession> | undefined

  const refresh = (): Promise<SpaSession> => {
    // One refresh per tab at a time: a burst of API calls rotates the token once.
    inflight ??= (async () => {
      const user = await manager.signinSilent()
      if (!user) throw new Error('no active session to refresh')
      return toSpaSession(user, now())
    })()
    return inflight.finally(() => {
      inflight = undefined
    })
  }

  return {
    async signIn() {
      await manager.signinRedirect()
    },

    async completeSignIn(url) {
      const user = await manager.signinRedirectCallback(url)
      return toSpaSession(user, now())
    },

    async currentSession() {
      const user = await manager.getUser()
      return user ? toSpaSession(user, now()) : undefined
    },

    refresh,

    async accessToken() {
      const user = await manager.getUser()
      if (user && !expired(user, now())) return user.access_token
      return (await refresh()).accessToken
    },

    // Local logout clears only this tab's session.
    async signOutLocal() {
      await manager.removeUser()
    },

    // Central logout leaves the page for the standard end-session endpoint.
    async signOutCentral() {
      await manager.signoutRedirect()
    },
  }
}
