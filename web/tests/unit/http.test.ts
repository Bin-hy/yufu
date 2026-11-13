import { afterEach, it, expect, vi } from 'vitest'
import axios, { type InternalAxiosRequestConfig } from 'axios'
import { createHttpClient } from '@/http/axios'
import { resetSession } from '@/workspace/session'
import { useToastStore } from '@/pinia/stores/toast'
import { pinia } from '@/pinia'
afterEach(() => vi.unstubAllEnvs())

it.each([undefined, '', '   '])('未配置 API 地址 %j 时，登录使用同源 /api/v1', async (baseURL) => {
    vi.stubEnv('VITE_BASE_URL', baseURL)
    const adapter = vi.fn(async (config: InternalAxiosRequestConfig) => ({
        data: config.url === 'csrf' ? { csrf_token: 'test-bootstrap' } : { access_token: 'test-only' },
        status: 200,
        statusText: 'OK',
        headers: { 'content-type': 'application/json' },
        config,
    }))
    const request = createHttpClient({ adapter })
    await expect(request({ url: 'login', method: 'POST' })).resolves.toEqual({
        access_token: 'test-only',
    })
    const login = adapter.mock.calls.find(([config]) => config.url === 'login')![0]
    expect(axios.getUri(login)).toBe('/api/v1/login')
    expect(login.headers.get('X-CSRF-Token')).toBe('test-bootstrap')
})

it('自定义 API 地址对登录和自动刷新一致生效', async () => {
    vi.stubEnv('VITE_BASE_URL', 'https://auth.example.test/custom/api')
    const urls: string[] = []
    const request = createHttpClient({
        adapter: async (config) => {
            urls.push(axios.getUri(config))
            if (config.url === 'csrf') return { data: { csrf_token: 'test-bootstrap' }, status: 200, statusText: 'OK', headers: {}, config }
            if (config.url !== 'refresh' && !(config as { _retry?: boolean })._retry)
                return unauthorized(config)
            return {
                data: config.url === 'refresh' ? { access_token: 'test-refresh' } : { ok: true },
                status: 200,
                statusText: 'OK',
                headers: {},
                config,
            }
        },
    })
    await expect(request({ url: 'users', errorPresentation: 'local' })).resolves.toEqual({ ok: true })
    expect(urls).toEqual([
        'https://auth.example.test/custom/api/users',
        'https://auth.example.test/custom/api/csrf',
        'https://auth.example.test/custom/api/refresh',
        'https://auth.example.test/custom/api/users',
    ])
})

it('未配置 API 地址时，自动刷新也使用 /api/v1', async () => {
    vi.stubEnv('VITE_BASE_URL', undefined)
    const urls: string[] = []
    const request = createHttpClient({
        adapter: async (config) => {
            urls.push(axios.getUri(config))
            if (config.url === 'csrf') return { data: { csrf_token: 'test-bootstrap' }, status: 200, statusText: 'OK', headers: {}, config }
            if (config.url !== 'refresh' && !(config as { _retry?: boolean })._retry)
                return unauthorized(config)
            return {
                data: config.url === 'refresh' ? { access_token: 'test-refresh' } : { ok: true },
                status: 200,
                statusText: 'OK',
                headers: {},
                config,
            }
        },
    })
    await request({ url: 'users', errorPresentation: 'local' })
    expect(urls).toEqual(['/api/v1/users', '/api/v1/csrf', '/api/v1/refresh', '/api/v1/users'])
})

it.each([200, 502])('接口返回 %i HTML 时显示地址错误，不把页面当成业务数据', async (status) => {
    const request = createHttpClient({
        adapter: async (config) => {
            if (config.url === 'csrf') return { data: { csrf_token: 'test-bootstrap' }, status: 200, statusText: 'OK', headers: {}, config }
            const response = {
                data: '<!doctype html><html><body>SPA fallback</body></html>',
                status,
                statusText: status === 200 ? 'OK' : 'Bad Gateway',
                headers: { 'content-type': 'text/html; charset=utf-8' },
                config,
            }
            if (status !== 200)
                throw new axios.AxiosError('Bad Gateway', 'ERR_BAD_RESPONSE', config, {}, response)
            return response
        },
    })
    await expect(request({ url: 'login', method: 'POST', errorPresentation: 'local' })).rejects.toThrow(
        '接口返回了 HTML 页面，请检查 API 地址或代理配置',
    )
})

it('invalid_input 400 转换为带请求标识的中文提示', async () => {
    const request = createHttpClient({
        adapter: async (config) => {
            if (config.url === 'csrf') return response(config, { csrf_token: 'test-bootstrap' })
            throw new axios.AxiosError('Bad Request', 'ERR_BAD_REQUEST', config, {},
                { data: { code: 'invalid_input', request_id: 'bb1a7a3e-0808-42f5-b4bd-e161fa884aea' }, status: 400, statusText: 'Bad Request', headers: {}, config })
        },
    })
    await expect(request({ url: 'users', method: 'POST', errorPresentation: 'local' })).rejects.toThrow(
        '输入内容不符合要求，请按表单提示检查后重试（请求标识 bb1a7a3e-0808-42f5-b4bd-e161fa884aea）',
    )
})

function unauthorized(config: InternalAxiosRequestConfig) {
    return Promise.reject(
        new axios.AxiosError(
            'Unauthorized',
            'ERR_BAD_REQUEST',
            config,
            {},
            { data: 'expired', status: 401, statusText: 'Unauthorized', headers: {}, config },
        ),
    )
}

function response(config: InternalAxiosRequestConfig, data: unknown) {
    return { data, status: 200, statusText: 'OK', headers: {}, config }
}

it.each([503, 200])('CSRF 返回 %i 故障时只显示一次全局错误', async (status) => {
    const store = useToastStore(pinia)
    store.clear()
    const request = createHttpClient({ adapter: async (config) => {
        const result = { ...response(config, status === 200 ? '<html>bad API</html>' : 'security dependency unavailable'), status,
            headers: { 'content-type': status === 200 ? 'text/html' : 'text/plain' } }
        if (status === 503) throw new axios.AxiosError('Unavailable', 'ERR_BAD_RESPONSE', config, {}, result)
        return result
    } })
    await expect(request({ url: 'login', method: 'POST' })).rejects.toThrow()
    expect(store.toasts).toHaveLength(1)
    store.clear()
})

it('只读接口 403 不丢弃有效 CSRF 证明', async () => {
    let challenges = 0
    const request = createHttpClient({ adapter: async (config) => {
        if (config.url === 'csrf') {
            challenges++
            return response(config, { csrf_token: 'still-valid-proof' })
        }
        if (config.url === 'restricted') {
            const result = { ...response(config, 'forbidden'), status: 403 }
            throw new axios.AxiosError('Forbidden', 'ERR_BAD_REQUEST', config, {}, result)
        }
        expect(config.headers.get('X-CSRF-Token')).toBe('still-valid-proof')
        return response(config, {})
    } })
    await request({ url: 'first', method: 'POST' })
    await expect(request({ url: 'restricted', errorPresentation: 'local' })).rejects.toThrow('权限')
    await request({ url: 'second', method: 'POST' })
    expect(challenges).toBe(1)
})

it.each([
    ['csrf_origin_not_allowed', '当前访问地址未获授权'],
    ['csrf_cookie_requires_https', '--allow-insecure-loopback-cookies'],
    ['csrf_cookie_missing', '浏览器未发送认证 Cookie'],
    ['csrf_proof_missing', '强制刷新页面'],
    ['csrf_proof_mismatch', '认证校验信息已变化'],
    ['csrf_proof_invalid', '认证校验信息已失效'],
])('认证拒绝 %s 给出明确提示且不自动重放登录', async (code, expected) => {
    const urls: string[] = []
    const request = createHttpClient({ adapter: async (config) => {
        urls.push(String(config.url))
        if (config.url === 'csrf') return response(config, { csrf_token: 'proof' })
        const result = { ...response(config, { code, message: 'forbidden' }), status: 403 }
        throw new axios.AxiosError('Forbidden', 'ERR_BAD_REQUEST', config, {}, result)
    } })
    await expect(request({ url: 'login', method: 'POST', errorPresentation: 'local' })).rejects.toThrow(expected)
    expect(urls).toEqual(['csrf', 'login'])
})

it('并发写请求共用 CSRF 获取，取消的请求不会提交', async () => {
    let release!: (data: { csrf_token: string }) => void
    let challenges = 0
    const submitted: string[] = []
    const request = createHttpClient({ adapter: async (config) => {
        if (config.url === 'csrf') {
            challenges++
            return response(config, await new Promise((resolve) => { release = resolve }))
        }
        expect(config.headers.get('X-CSRF-Token')).toBe('concurrent-proof')
        submitted.push(config.url!)
        return response(config, { ok: true })
    } })
    const controller = new AbortController()
    const a = request({ url: 'a', method: 'POST' })
    const b = request({ url: 'b', method: 'DELETE', signal: controller.signal }).catch((error) => error)
    const c = request({ url: 'c', method: 'PATCH' })
    await vi.waitFor(() => expect(challenges).toBe(1))
    controller.abort()
    release({ csrf_token: 'concurrent-proof' })
    await Promise.all([a, c])
    expect(axios.isCancel(await b)).toBe(true)
    expect(submitted.sort()).toEqual(['a', 'c'])
})

it('登录与刷新返回的新 CSRF 用于后续写请求，退出后重新获取', async () => {
    const calls: [string, unknown][] = []
    const request = createHttpClient({ adapter: async (config) => {
        calls.push([config.url!, config.headers.get('X-CSRF-Token')])
        return response(config, config.url === 'csrf' ? { csrf_token: 'bootstrap' }
            : config.url === 'login' ? { csrf_token: 'login-proof', access_token: 'test-login' }
            : config.url === 'refresh' ? { csrf_token: 'refresh-proof', access_token: 'test-refresh' }
            : { ok: true })
    } })
    for (const url of ['login', 'profile', 'refresh', 'logout', 'other'])
        await request({ url, method: 'POST' })
    expect(calls).toEqual([
        ['csrf', undefined], ['login', 'bootstrap'], ['profile', 'login-proof'],
        ['refresh', 'login-proof'], ['logout', 'refresh-proof'], ['csrf', undefined], ['other', 'bootstrap'],
    ])
})

it('旧会话的 CSRF 响应取消，不覆盖新会话证明或提交旧写请求', async () => {
    const releases: ((token: string) => void)[] = []
    const submitted: string[] = []
    const request = createHttpClient({ adapter: async (config) => {
        if (config.url === 'csrf') {
            const token = await new Promise<string>((resolve) => releases.push(resolve))
            return response(config, { csrf_token: token })
        }
        expect(config.headers.get('X-CSRF-Token')).toBe('new-proof')
        submitted.push(config.url!)
        return response(config, { ok: true })
    } })
    const old = request({ url: 'old', method: 'POST' }).catch((error) => error)
    await vi.waitFor(() => expect(releases).toHaveLength(1))
    resetSession()
    const fresh = request({ url: 'fresh', method: 'POST' })
    await vi.waitFor(() => expect(releases).toHaveLength(2))
    releases[1]!('new-proof')
    await fresh
    releases[0]!('old-proof')
    expect(axios.isCancel(await old)).toBe(true)
    await request({ url: 'next', method: 'DELETE' })
    expect(submitted).toEqual(['fresh', 'next'])
})

it('CSRF 获取失败保留明确的 503 信息，不提交或刷新登录请求', async () => {
    const urls: string[] = []
    const request = createHttpClient({ adapter: async (config) => {
        urls.push(config.url!)
        throw new axios.AxiosError('unavailable', 'ERR_BAD_RESPONSE', config, {}, {
            data: 'security dependency unavailable', status: 503, statusText: 'Unavailable', headers: {}, config,
        })
    } })
    await expect(request({ url: 'login', method: 'POST', errorPresentation: 'local' })).rejects.toThrow(
        '认证服务暂不可用，请检查后端时钟同步与依赖状态',
    )
    expect(urls).toEqual(['csrf'])
})

it('CSRF 的 401 不递归刷新，写请求的 403 不自动重放', async () => {
    let failChallenge = true
    const urls: string[] = []
    const request = createHttpClient({ adapter: async (config) => {
        urls.push(config.url!)
        if (config.url === 'csrf' && failChallenge) return unauthorized(config)
        if (config.url === 'csrf') return response(config, { csrf_token: 'proof' })
        throw new axios.AxiosError('forbidden', 'ERR_BAD_REQUEST', config, {}, {
            data: 'operation forbidden', status: 403, statusText: 'Forbidden', headers: {}, config,
        })
    } })
    await expect(request({ url: 'profile', method: 'PATCH', errorPresentation: 'local' })).rejects.toThrow()
    expect(urls).toEqual(['csrf'])
    failChallenge = false
    await expect(request({ url: 'profile', method: 'PATCH', errorPresentation: 'local' })).rejects.toThrow()
    expect(urls).toEqual(['csrf', 'csrf', 'profile'])
    await expect(request({ url: 'profile', method: 'PATCH', errorPresentation: 'local' })).rejects.toThrow()
    expect(urls).toEqual(['csrf', 'csrf', 'profile', 'csrf', 'profile'])
})
it('并发 401 共用一次刷新，取消的等待请求不重放', async () => {
    let release!: (token: string) => void
    const refresh = vi.fn(
            () =>
                new Promise<string>((resolve) => {
                    release = resolve
                }),
        ),
        replay: string[] = []
    const adapter = vi.fn((config: InternalAxiosRequestConfig) => {
        if (!(config as InternalAxiosRequestConfig & { _retry?: boolean })._retry)
            return unauthorized(config)
        replay.push(config.url!)
        return Promise.resolve({
            data: config.url,
            status: 200,
            statusText: 'OK',
            headers: {},
            config,
        })
    })
    const request = createHttpClient({ adapter, refresh }),
        controller = new AbortController()
    const a = request({ url: '/a', errorPresentation: 'local' }),
        b = request({ url: '/b', signal: controller.signal, errorPresentation: 'local' }).catch(
            (e) => e,
        ),
        c = request({ url: '/c', errorPresentation: 'local' })
    await vi.waitFor(() => expect(refresh).toHaveBeenCalledTimes(1))
    controller.abort()
    release('new')
    await expect(a).resolves.toBe('/a')
    await expect(c).resolves.toBe('/c')
    expect(axios.isCancel(await b)).toBe(true)
    expect(replay.sort()).toEqual(['/a', '/c'])
})
it('刷新失败终结全部等待请求，不产生悬挂订阅', async () => {
    const request = createHttpClient({
        adapter: unauthorized,
        refresh: async () => {
            throw Error('refresh failed')
        },
    })
    const values = await Promise.allSettled([request({ url: '/a' }), request({ url: '/b' })])
    expect(values.every((value) => value.status === 'rejected')).toBe(true)
})
it('旧会话的成功响应被取消', async () => {
    let release!: () => void
    const request = createHttpClient({
        adapter: (config) =>
            new Promise((resolve) => {
                release = () =>
                    resolve({ data: 'old', status: 200, statusText: 'OK', headers: {}, config })
            }),
    })
    const result = request({ url: '/a' }).catch((e) => e)
    await vi.waitFor(() => expect(release).toBeDefined())
    resetSession()
    release()
    expect(axios.isCancel(await result)).toBe(true)
})

it('结构化异常保留响应，local 不弹 Toast，全局同原因并发去重', async () => {
    const store = useToastStore(pinia)
    store.clear()
    let id = 0
    const request = createHttpClient({ adapter: async config => {
        const result = { ...response(config, { code: 'owner_required', request_id: `req-${++id}` }), status: 409 }
        throw new axios.AxiosError('raw error', 'ERR_BAD_REQUEST', config, {}, result)
    } })
    const local = await request({ url: 'admin/accounts', errorPresentation: 'local' }).catch(e => e)
    if (!axios.isAxiosError(local)) throw Error('预期保留 Axios 异常')
    expect(local.response.status).toBe(409)
    expect(local.response.data.code).toBe('owner_required')
    expect(local.message).toContain('转交负责人')
    expect(store.toasts).toHaveLength(0)
    await Promise.allSettled([request({ url: 'one' }), request({ url: 'two' })])
    expect(store.toasts).toHaveLength(1)
    expect(store.toasts[0]?.message).toContain('转交负责人')
    store.clear()
})
