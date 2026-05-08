import { createRequest } from '@cobotweb/shared/http'
import { clearAuth, getAccessToken, redirectToLogin, refreshAccessToken } from './auth'

// All cobotrix-bound API calls go through cobotsrv's /mgmt/* proxy.
// cobotsrv verifies the JWT once at the proxy layer, then cobotrix re-verifies
// via JWKS — defense in depth.
export const request = createRequest({
  baseURL: import.meta.env.VITE_COBOTSRV_URL ?? '',
  getToken: () => getAccessToken() ?? undefined,
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
