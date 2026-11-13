import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import axios, { type InternalAxiosRequestConfig } from 'axios'
import { createHttpClient } from '@/http/axios'
import { setToken, getToken } from '@/common/cache/cookies'
import { resetSession, sessionEpoch, onSessionReset } from '@/workspace/session'
import { useUserStore } from '@/pinia/stores/user'
import { pinia } from '@/pinia'
import { canEnter, RegistryRouterGuard } from '@/router/guard'
import { createRouter, createMemoryHistory, isNavigationFailure } from 'vue-router'
const mocked = vi.hoisted(() => ({ getCenterSession: vi.fn() }))
vi.mock('@/pages/account/api', () => ({ getCenterSession: mocked.getCenterSession }))
const identity = { user_id: 12, sub: '12', username: 'employee', nickname: 'Employee', person_kind: 'employee' as const, developer_enabled: true, administrator: false, sso_session_id: 'root-one', auth_time: 10, expires_at: 5000, csrf_token: 'center-proof' }
beforeEach(() => { resetSession(); mocked.getCenterSession.mockReset() })
afterEach(() => vi.unstubAllEnvs())
const response = (config: InternalAxiosRequestConfig, data: unknown) => ({ data, status: 200, statusText: 'OK', headers: {}, config })
it('中心请求固定同源、只发 Cookie 和中心 CSRF，不发送旧 Bearer', async () => {
    vi.stubEnv('VITE_BASE_URL', 'https://external.test/api')
    setToken('legacy-only')
    const calls: InternalAxiosRequestConfig[] = []
    const request = createHttpClient({ authMode: 'center', adapter: async config => {
        calls.push(config)
        expect(config.headers.get('Authorization')).toBeUndefined()
        expect(config.withCredentials).toBe(true)
        return response(config, config.url === 'identity/csrf' ? { csrf_token: 'anonymous-proof' } : identity)
    } })
    await request({ url: 'identity/login', method: 'POST' })
    await request({ url: 'account/profile', method: 'PATCH', headers: { Authorization: 'caller-legacy' } })
    expect(calls.map(config => axios.getUri(config))).toEqual(['/api/v1/identity/csrf', '/api/v1/identity/login', '/api/v1/account/profile'])
    expect(calls[2]!.headers.get('X-CSRF-Token')).toBe('center-proof')
    await expect(request({ url: 'https://external.test/api' })).rejects.toThrow('同源')
    await expect(request({ url: 'account/session', baseURL: '//external.test' })).rejects.toThrow('同源')
    expect(calls).toHaveLength(3)
})
it.each([401, 403])('中心 %i 不尝试刷新或重放，403 保留当前会话', async status => {
    const store = useUserStore(pinia); store.setCenterSession(identity)
    const initial = sessionEpoch.value
    const refresh = vi.fn()
    const request = createHttpClient({ authMode: 'center', refresh, adapter: async config => {
        throw new axios.AxiosError('denied', 'ERR_BAD_REQUEST', config, {}, { ...response(config, { message: 'denied' }), status })
    } })
    await expect(request({ url: 'account/session', errorPresentation: 'local' })).rejects.toThrow()
    expect(refresh).not.toHaveBeenCalled()
    expect(sessionEpoch.value > initial).toBe(status === 401)
    expect(store.centerSession !== null).toBe(status === 403)
})
it('丢弃存储标签和旧令牌，多个恢复请求共用服务端结果', async () => {
    const store = useUserStore(pinia)
    setToken('old-token'); localStorage.setItem('userInfo', '{"username":"admin"}')
    store.initUserInfo()
    expect(getToken()).toBeUndefined(); expect(store.isLoggedIn).toBe(false)
    let resolve!: (value: typeof identity) => void
    mocked.getCenterSession.mockImplementation(() => new Promise(r => { resolve = r }))
    const requests = [store.ensureSession(), store.ensureSession(), store.ensureSession()]
    expect(mocked.getCenterSession).toHaveBeenCalledTimes(1)
    resolve(identity); await Promise.all(requests)
    expect(store.centerSession?.sub).toBe('12'); expect(store.loading).toBe(false)
    expect(localStorage.getItem('userInfo')).not.toContain('Employee')
})
it('资格和根变化清缓存，旧响应不能恢复已经退出的身份', async () => {
    const store = useUserStore(pinia); store.setCenterSession(identity)
    const reset = vi.fn(); const unregister = onSessionReset(reset)
    try {
        store.setCenterSession({ ...identity, developer_enabled: false })
        expect(reset).toHaveBeenCalledTimes(1)
        expect(canEnter({ qualification: 'developer' }, store.centerSession)).toBe(false)
        expect(canEnter({ qualification: 'administrator' }, store.centerSession)).toBe(false)
        expect(canEnter({}, store.centerSession)).toBe(true)
        let resolve!: (value: typeof identity) => void
        mocked.getCenterSession.mockImplementation(() => new Promise(r => { resolve = r }))
        const loading = store.ensureSession(); store.logout(); resolve(identity); await loading
        expect(store.centerSession).toBeNull(); expect(store.loading).toBe(false)
    } finally { unregister() }
})
it('身份查询网络故障保留可重试错误，401 清除身份', async () => {
    const store = useUserStore(pinia); store.setCenterSession(identity)
    mocked.getCenterSession.mockRejectedValue(new Error('temporarily unavailable'))
    await store.ensureSession(); expect(store.identityError).toBe('temporarily unavailable')
    mocked.getCenterSession.mockRejectedValue({ response: { status: 401 }, message: 'expired' })
    await store.ensureSession(); expect(store.centerSession).toBeNull()
})

async function sessionRouter() {
    const router = createRouter({ history: createMemoryHistory(), routes: [
        { path: '/one', name: 'one', component: {}, meta: { requiresAuth: true } },
        { path: '/two', name: 'two', component: {}, meta: { requiresAuth: true } },
        { path: '/login', name: 'login', component: {}, meta: { requiresAuth: false } },
        { path: '/403', name: '403', component: {}, meta: { requiresAuth: false } },
    ] })
    RegistryRouterGuard(router)
    return router
}
it('已登录时身份服务临时故障中止切页，保留原页面与会话，不跳登录', async () => {
    mocked.getCenterSession.mockResolvedValue(identity)
    const router = await sessionRouter()
    await router.push('/one')
    mocked.getCenterSession.mockRejectedValue({ response: { status: 503 }, message: '身份服务暂不可用' })
    const failure = await router.push('/two')
    expect(isNavigationFailure(failure)).toBe(true)
    expect(router.currentRoute.value.path).toBe('/one')
    expect(useUserStore(pinia).centerSession?.sso_session_id).toBe('root-one')
    mocked.getCenterSession.mockResolvedValue(identity)
    await router.push('/two')
    expect(router.currentRoute.value.path).toBe('/two')
})
it('初次恢复故障可重试，真正 401 才要求重新登录', async () => {
    const router = await sessionRouter()
    mocked.getCenterSession.mockRejectedValue(new Error('连接超时'))
    await router.push('/one')
    expect(router.currentRoute.value.path).toBe('/login')
    expect(useUserStore(pinia).identityError).toBe('连接超时')
    mocked.getCenterSession.mockRejectedValue({ response: { status: 401 }, message: 'expired' })
    await router.push('/one')
    expect(router.currentRoute.value.path).toBe('/login')
    expect(useUserStore(pinia).identityError).toBeNull()
    expect(useUserStore(pinia).centerSession).toBeNull()
})

it('并发中心 401 只触发一次过期恢复，主动退出和匿名访问无过期提示', async () => {
    const { setAuthExpiredHandler, sessionExpiredNotice } = await import('@/workspace/session')
    mocked.getCenterSession.mockResolvedValue(identity)
    const router = await sessionRouter()
    await router.push('/one')
    const expired = vi.fn(() => { void router.replace('/login') })
    setAuthExpiredHandler(expired)
    const store = useUserStore(pinia)
    store.setCenterSession(identity)
    const releases: (() => void)[] = []
    const request = createHttpClient({ authMode: 'center', adapter: config => new Promise((_resolve, reject) => {
        releases.push(() => reject(new axios.AxiosError('denied', 'ERR_BAD_REQUEST', config, {}, { ...response(config, { code: 'unauthenticated' }), status: 401 })))
    }) })
    const results = [request({ url: 'account/profile', errorPresentation: 'local' }), request({ url: 'account/session', errorPresentation: 'local' })].map(p => p.catch(e => e))
    await vi.waitFor(() => expect(releases).toHaveLength(2))
    releases.forEach(release => release()); await Promise.all(results)
    await vi.waitFor(() => expect(router.currentRoute.value.path).toBe('/login'))
    expect(expired).toHaveBeenCalledTimes(1)
    expect(sessionExpiredNotice.value).toBe(true)
    expect(store.centerSession).toBeNull()
    store.logout()
    expect(sessionExpiredNotice.value).toBe(false)
    const anonymous = request({ url: 'account/session', errorPresentation: 'local' }).catch(e => e)
    await vi.waitFor(() => expect(releases).toHaveLength(3))
    releases[2]!(); await anonymous
    expect(sessionExpiredNotice.value).toBe(false)
    store.setCenterSession(identity)
    expect(sessionExpiredNotice.value).toBe(false)
    setAuthExpiredHandler(() => {})
})
