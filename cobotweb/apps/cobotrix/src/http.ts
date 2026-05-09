import { createRequest } from '@cobotweb/shared/http'
import { clearAuth, getAccessToken, redirectToLogin, refreshAccessToken } from './auth'

// API calls go directly to cobotrix. cobotrix independently verifies the JWT
// (RS256 via JWKS) and enforces route-level require_perm! checks.
// OIDC flows (authorize, token, refresh) go to cobotsrv — see auth.ts.
export const request = createRequest({
  baseURL: import.meta.env.VITE_COBOTRIX_URL ?? 'http://localhost:4001',
  getToken: () => getAccessToken() ?? undefined,
  setToken: (_token: string) => {
    // refreshAccessToken() already calls storeTokens()
  },
  refreshToken: async () => {
    const newToken = await refreshAccessToken()
    if (!newToken) throw new Error('refresh failed')
    return newToken
  },
  onUnauthorized: () => {
    clearAuth()
    void redirectToLogin(window.location.pathname + window.location.search)
  },
})
