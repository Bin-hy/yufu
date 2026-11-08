import { afterEach, expect, it, vi } from 'vitest'
import Cookies from 'js-cookie'
import { CacheKey } from '@/common/constants/cache-key'

afterEach(() => vi.restoreAllMocks())

it('长 JWT 在当前标签页保存，刷新可恢复，退出完整清除', async () => {
    let cache = await import('@/common/cache/cookies')
    const token = 'test-jwt.' + 'x'.repeat(7400) + '.signature'
    cache.setToken(token)
    expect(cache.getToken()).toBe(token)
    expect(Cookies.get(CacheKey.ACCESS_TOKEN)).toBeUndefined()
    expect(JSON.stringify(localStorage)).not.toContain(token)
    expect(sessionStorage.getItem(`cobotusr:access:v1:${CacheKey.ACCESS_TOKEN}`)).toBe(token)
    vi.resetModules()
    cache = await import('@/common/cache/cookies')
    expect(cache.getToken()).toBe(token)
    cache.removeToken()
    expect(cache.getToken()).toBeUndefined()
    expect(sessionStorage.getItem(`cobotusr:access:v1:${CacheKey.ACCESS_TOKEN}`)).toBeNull()
})

it('浏览器拒绝存储时，内存登录和退出仍可用', async () => {
    const cache = await import('@/common/cache/cookies')
    const fail = () => { throw new DOMException('Storage blocked', 'SecurityError') }
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(fail)
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(fail)
    vi.spyOn(Storage.prototype, 'removeItem').mockImplementation(fail)
    cache.setToken('in-memory-token')
    expect(cache.getToken()).toBe('in-memory-token')
    expect(() => cache.removeToken()).not.toThrow()
    expect(cache.getToken()).toBeUndefined()
})

it('已有短 Token Cookie 可迁移且不再随 API 请求附带', async () => {
    vi.resetModules()
    const cache = await import('@/common/cache/cookies')
    Cookies.set(CacheKey.ACCESS_TOKEN, 'legacy-test-token')
    expect(cache.getToken()).toBe('legacy-test-token')
    expect(Cookies.get(CacheKey.ACCESS_TOKEN)).toBeUndefined()
    cache.removeToken()
})
