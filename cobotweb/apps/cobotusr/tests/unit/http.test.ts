import { it, expect, vi } from 'vitest'
import axios, { type InternalAxiosRequestConfig } from 'axios'
import { createHttpClient } from '@/http/axios'
import { resetSession } from '@/workspace/session'
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
