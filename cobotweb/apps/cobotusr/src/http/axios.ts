import axios, { type AxiosRequestConfig, type AxiosInstance } from 'axios'
import { getToken, setToken } from '@/common/cache/cookies'
import { emitToast } from '@/utils/toast'
import { expireSession, sessionEpoch } from '@/workspace/session'
export interface RequestOptions extends AxiosRequestConfig {
    errorPresentation?: 'local' | 'global'
}
export function createHttpClient(options?: {
    adapter?: AxiosRequestConfig['adapter']
    refresh?: () => Promise<string>
}) {
    const instance: AxiosInstance = axios.create({ adapter: options?.adapter })
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
            return response.data
        },
        async (error) => {
            if (axios.isCancel(error) || error?.code === 'ERR_CANCELED') throw error
            const original = (error.config ?? {}) as RequestOptions & {
                _retry?: boolean
                _epoch?: number
            }
            const epoch = original._epoch ?? sessionEpoch.value
            if (epoch !== sessionEpoch.value) throw new axios.CanceledError('会话已改变')
            const status = error.response?.status
            const url = String(original.url ?? '')
            if (status === 401 && !/login$/i.test(url)) {
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
                                      baseURL: import.meta.env.VITE_BASE_URL,
                                      withCredentials: true,
                                      _retry: true,
                                      _epoch: epoch,
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
            const message =
                typeof error.response?.data === 'string'
                    ? error.response.data
                    : (error.response?.data?.message ?? error.message ?? '请求失败，请重试')
            error.message = message
            if (original.errorPresentation !== 'local') emitToast({ type: 'error', message })
            throw error
        },
    )
    return function request<T>(config: RequestOptions): Promise<T> {
        return instance.request({
            baseURL: import.meta.env.VITE_BASE_URL,
            timeout: 8000,
            withCredentials: true,
            ...config,
            headers: {
                'Content-Type': 'application/json',
                Authorization: getToken() ? 'Bearer ' + getToken() : undefined,
                ...config.headers,
            },
            _epoch: sessionEpoch.value,
        } as AxiosRequestConfig) as Promise<T>
    }
}
export const request = createHttpClient()
