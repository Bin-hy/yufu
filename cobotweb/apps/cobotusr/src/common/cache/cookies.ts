import { CacheKey } from '@/common/constants/cache-key'
import Cookies from 'js-cookie'

// Permission-bearing JWTs exceed a single cookie's capacity. Keep access tokens
// in this tab; refresh cookies remain server-owned and HttpOnly.
const storageKey = `cobotusr:access:v1:${CacheKey.ACCESS_TOKEN}`
let accessToken: string | undefined
let cleared = false

export function getToken(): string | undefined {
    if (cleared) return undefined
    if (accessToken) return accessToken
    try {
        accessToken = sessionStorage.getItem(storageKey) || undefined
    } catch {
        /* This tab remains usable when browser storage is unavailable. */
    }
    if (accessToken) return accessToken
    const legacy = Cookies.get(CacheKey.ACCESS_TOKEN)
    if (legacy) setToken(legacy)
    return accessToken
}

export function setToken(token: string) {
    cleared = false
    accessToken = token
    try {
        sessionStorage.setItem(storageKey, token)
    } catch {
        /* Keep the active token in memory. */
    }
    Cookies.remove(CacheKey.ACCESS_TOKEN)
}

export function removeToken() {
    cleared = true
    accessToken = undefined
    try {
        sessionStorage.removeItem(storageKey)
    } catch {
        /* The active session is still cleared in memory. */
    }
    Cookies.remove(CacheKey.ACCESS_TOKEN)
}
