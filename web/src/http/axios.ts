import axios, { type AxiosRequestConfig, type AxiosInstance, type InternalAxiosRequestConfig } from 'axios'
import { getToken, setToken } from '@/common/cache/cookies'
import { emitToast } from '@/utils/toast'
import { expireSession, sessionEpoch } from '@/workspace/session'
import { formatFailure, normalizeFailure, isHtmlResponse } from './errors'
export interface RequestOptions extends AxiosRequestConfig {
    errorPresentation?: 'local' | 'global'
}
type InternalRequest = InternalAxiosRequestConfig & {
    _retry?: boolean
    _epoch?: number
    _skipAuthRefresh?: boolean
    errorPresentation?: 'local' | 'global'
}
const csrfLifetime = 5 * 60 * 1000
const htmlResponseMessage = '接口返回了 HTML 页面，请检查 API 地址或代理配置'
export function createHttpClient(options?: {
    adapter?: AxiosRequestConfig['adapter']
    refresh?: () => Promise<string>
    authMode?: 'legacy' | 'center'
}) {
    const center = options?.authMode === 'center'
    const baseURL = center ? '/api/v1' : import.meta.env.VITE_BASE_URL?.trim() || '/api/v1'
    const instance: AxiosInstance = axios.create({ adapter: options?.adapter })
    const proofs = new Map<string, { token: string; epoch: number; expiresAt: number }>()
    const challenges = new Map<string, { epoch: number; promise: Promise<string> }>()
    const presentedErrors = new WeakSet<object>()
    function presentError(error: object, config: RequestOptions, message: string) {
        if (config.errorPresentation !== 'local' && !presentedErrors.has(error)) {
            presentedErrors.add(error)
            const failure = normalizeFailure(error)
            emitToast({ type: 'error', message, dedupeKey: failure.code, durationMs: failure.outcomeUncertain ? 0 : undefined })
        }
    }
    function proofKey(config: AxiosRequestConfig) {
        return config.baseURL ?? baseURL
    }
    function rememberProof(config: InternalRequest, data: unknown) {
        const token = (data as { csrf_token?: unknown } | null)?.csrf_token
        if (typeof token === 'string' && token.trim())
            proofs.set(proofKey(config), { token, epoch: config._epoch ?? sessionEpoch.value, expiresAt: Date.now() + csrfLifetime })
    }
    async function csrfProof(config: InternalRequest, epoch: number): Promise<string> {
        const key = proofKey(config)
        const saved = proofs.get(key)
        if (saved?.epoch === epoch && saved.expiresAt > Date.now()) return saved.token
        const pending = challenges.get(key)
        if (pending?.epoch === epoch) return pending.promise
        const promise = instance.request({
            baseURL: key,
            url: center ? 'identity/csrf' : 'csrf',
            method: 'GET',
            timeout: 8000,
            withCredentials: true,
            errorPresentation: config.errorPresentation,
            _epoch: epoch,
            _skipAuthRefresh: true,
        } as AxiosRequestConfig).then((data: unknown) => {
            const token = (data as { csrf_token?: unknown } | null)?.csrf_token
            if (typeof token !== 'string' || !token.trim()) throw new Error('认证服务未返回 CSRF 证明，请检查后端配置')
            return token
        }).finally(() => {
            if (challenges.get(key)?.promise === promise) challenges.delete(key)
        })
        challenges.set(key, { epoch, promise })
        return promise
    }
    instance.interceptors.request.use(async (raw) => {
        const config = raw as InternalRequest
        const epoch = config._epoch ?? sessionEpoch.value
        config._epoch = epoch
        if (epoch !== sessionEpoch.value || config.signal?.aborted)
            throw new axios.CanceledError('会话已改变或请求已取消')
        if (['post', 'put', 'patch', 'delete'].includes(String(config.method).toLowerCase())) {
            const proof = await csrfProof(config, epoch)
            if (epoch !== sessionEpoch.value || config.signal?.aborted)
                throw new axios.CanceledError('会话已改变或请求已取消')
            config.headers.set('X-CSRF-Token', proof)
        }
        return config
    })
    let refreshing: Promise<string> | null = null,
        refreshingEpoch = -1
    instance.interceptors.response.use(
        (response) => {
            if (
                (response.config as RequestOptions & { _epoch?: number })._epoch !== undefined &&
                (response.config as RequestOptions & { _epoch?: number })._epoch !==
                    sessionEpoch.value
            )
                throw new axios.CanceledError('会话已改变')
            if (isHtmlResponse(response)) {
                const error = new axios.AxiosError(
                    htmlResponseMessage,
                    'ERR_API_HTML_RESPONSE',
                    response.config,
                    response.request,
                    response,
                )
                presentError(error, response.config, htmlResponseMessage)
                throw error
            }
            const config = response.config as InternalRequest
            rememberProof(config, response.data)
            if (/(?:^|\/)logout(?:\?|$)/i.test(String(config.url))) proofs.delete(proofKey(config))
            return response.data
        },
        async (error) => {
            if (axios.isCancel(error) || error?.code === 'ERR_CANCELED') throw error
            const original = (error.config ?? {}) as RequestOptions & {
                _retry?: boolean
                _epoch?: number
                _skipAuthRefresh?: boolean
            }
            const epoch = original._epoch ?? sessionEpoch.value
            if (epoch !== sessionEpoch.value) throw new axios.CanceledError('会话已改变')
            const status = error.response?.status
            const url = String(original.url ?? '')
            if (center && status === 401 && /^(?:\/?(?:account|developer|admin)\/|\/?identity\/logout$|\/?identity\/flows\/[^/]+\/complete$)/i.test(url)) {
                expireSession()
                error.message = '中心会话已失效，请重新登录'
                throw error
            }
            if (['post', 'put', 'patch', 'delete'].includes(String(original.method).toLowerCase())
                && (status === 403 || !error.response))
                proofs.delete(proofKey(original))
            if (!center && status === 401 && !original._skipAuthRefresh && !/(?:login|register|csrf)$/i.test(url)) {
                if (original._retry || /refresh$/i.test(url)) {
                    expireSession()
                    throw error
                }
                if (!refreshing || refreshingEpoch !== epoch) {
                    refreshingEpoch = epoch
                    const current = (
                        options?.refresh
                            ? options.refresh()
                            : instance
                                  .request({
                                      url: 'refresh',
                                      method: 'POST',
                                      baseURL,
                                      withCredentials: true,
                                      _retry: true,
                                      _epoch: epoch,
                                      errorPresentation: original.errorPresentation,
                                  } as AxiosRequestConfig)
                                  .then((data: unknown) => {
                                      const token = (data as { access_token?: string }).access_token
                                      if (!token) throw new Error('刷新未返回凭据')
                                      return token
                                  })
                    )
                        .then((token) => {
                            if (epoch !== sessionEpoch.value)
                                throw new axios.CanceledError('会话已改变')
                            setToken(token)
                            return token
                        })
                        .catch((e) => {
                            if (epoch === sessionEpoch.value) expireSession()
                            throw e
                        })
                        .finally(() => {
                            if (refreshing === current) refreshing = null
                        })
                    refreshing = current
                }
                const token = await refreshing
                if (original.signal?.aborted || epoch !== sessionEpoch.value)
                    throw new axios.CanceledError('请求已取消')
                return instance.request({
                    ...original,
                    _retry: true,
                    headers: { ...original.headers, Authorization: 'Bearer ' + token },
                } as AxiosRequestConfig)
            }
            const message = formatFailure(normalizeFailure(error))
            error.message = message
            presentError(error, original, message)
            throw error
        },
    )
    return function request<T>(config: RequestOptions): Promise<T> {
        if (center && ((config.baseURL && config.baseURL !== baseURL)
            || /^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(config.url ?? '')))
            return Promise.reject(new Error('中心接口必须使用同源地址'))
        return instance.request({
            baseURL,
            timeout: 8000,
            withCredentials: true,
            ...config,
            headers: {
                'Content-Type': 'application/json',
                Authorization: !center && getToken() ? 'Bearer ' + getToken() : undefined,
                ...config.headers,
                ...(center ? { Authorization: undefined } : {}),
            },
            _epoch: sessionEpoch.value,
        } as AxiosRequestConfig) as Promise<T>
    }
}
export const request = createHttpClient()
