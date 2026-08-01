import axios, { type AxiosInstance, type AxiosRequestConfig } from 'axios'
import { merge } from 'lodash-es'

export interface RequestFactoryOptions {
  baseURL: string
  /** Returns the current bearer token (or undefined / empty for anonymous calls). */
  getToken: () => string | undefined
  /** Persists a refreshed token; called on a successful refresh. */
  setToken?: (token: string) => void
  /** Refresh the access token; resolve with the new token or reject to force logout. */
  refreshToken?: () => Promise<string>
  /** Called when refresh fails or no refresh handler is configured. Apps should redirect to /login here. */
  onUnauthorized?: () => void
  /** Hook for global error reporting (toast / log). Receives axios error message. */
  onError?: (message: string, status?: number) => void
  /** Default timeout in ms (default 8000). */
  timeoutMs?: number
}

/**
 * Build a typed `request` function shared between cobotweb apps.
 * Apps inject their own token storage / logout hook so this module stays
 * UI-framework agnostic.
 */
export function createRequest(opts: RequestFactoryOptions) {
  const instance = createInstance(opts)
  const timeout = opts.timeoutMs ?? 8000

  return <T>(config: AxiosRequestConfig): Promise<T> => {
    const token = opts.getToken()
    const defaultConfig: AxiosRequestConfig = {
      baseURL: opts.baseURL,
      headers: {
        Authorization: token ? `Bearer ${token}` : undefined,
        'Content-Type': 'application/json',
      },
      timeout,
      withCredentials: true,
    }
    return instance(merge(defaultConfig, config))
  }
}

function createInstance(opts: RequestFactoryOptions): AxiosInstance {
  const instance = axios.create()
  let isRefreshing = false
  let refreshPromise: Promise<string> | null = null
  const subscribers: Array<(token: string) => void> = []
  const subscribe = (cb: (token: string) => void) => subscribers.push(cb)
  const flush = (token: string) => {
    subscribers.forEach((cb) => cb(token))
    subscribers.length = 0
  }

  instance.interceptors.response.use(
    (resp) => resp.data,
    async (error) => {
      const status: number | undefined = error?.response?.status
      const originalConfig = (error?.config ?? {}) as AxiosRequestConfig & { _retry?: boolean }

      if (status === 401 && opts.refreshToken) {
        const reqUrl = String(originalConfig.url ?? '')
        if (/login$/i.test(reqUrl)) return Promise.reject(error)
        if (/refresh$/i.test(reqUrl)) {
          opts.onUnauthorized?.()
          return Promise.reject(error)
        }
        if (originalConfig._retry) {
          opts.onUnauthorized?.()
          return Promise.reject(error)
        }

        if (!isRefreshing) {
          isRefreshing = true
          refreshPromise = opts
            .refreshToken()
            .then((newToken) => {
              opts.setToken?.(newToken)
              isRefreshing = false
              flush(newToken)
              return newToken
            })
            .catch((e) => {
              isRefreshing = false
              opts.onUnauthorized?.()
              throw e
            })
        }

        return new Promise((resolve, reject) => {
          subscribe((newToken) => {
            originalConfig.headers = originalConfig.headers ?? {}
            ;(originalConfig.headers as Record<string, string>)['Authorization'] = `Bearer ${newToken}`
            originalConfig._retry = true
            resolve(instance(originalConfig))
          })
          refreshPromise?.catch(reject)
        })
      }

      if (status === 401 && !opts.refreshToken) {
        opts.onUnauthorized?.()
      }

      // Surface the server error body (plain string or { message } / { error })
      // on the rejected error so pages render something useful instead of
      // axios' generic "Request failed with status code 403".
      const data: unknown = error?.response?.data
      const serverMessage =
        typeof data === 'string' && data
          ? data
          : data && typeof data === 'object'
            ? ((data as Record<string, unknown>).message ?? (data as Record<string, unknown>).error)
            : undefined
      if (typeof serverMessage === 'string' && serverMessage) {
        error.message = serverMessage
      }

      const message =
        (typeof serverMessage === 'string' && serverMessage) ||
        error?.message ||
        '请求错误'
      opts.onError?.(message, status)
      return Promise.reject(error)
    },
  )

  return instance
}
