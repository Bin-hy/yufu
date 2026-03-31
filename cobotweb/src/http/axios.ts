import axios, { type AxiosInstance, type AxiosRequestConfig } from "axios"
import { getToken, setToken } from "@/common/cache/cookies"
import { get, merge } from "lodash-es"
import { useUserStore } from "@/pinia/stores/user"
import { emitToast } from "@/utils/toast"
import { router } from "@/router"
import { refreshTokenRequest } from "@/pages/login/api"

/** 退出登录并强制刷新页面（会重定向到登录页） */
function logout() {
  const userStore = useUserStore()
  userStore.logout()
  // 跳转到登录页面
  router.push('/login')
}

/** 创建请求实例 */
function createInstance() {
  // 创建一个 axios 实例命名为 instance
  const instance = axios.create()
  // 刷新令牌并发控制
  let isRefreshing = false
  let refreshPromise: Promise<string> | null = null
  const subscribers: Array<(token: string) => void> = []
  const subscribeTokenRefresh = (cb: (token: string) => void) => { subscribers.push(cb) }
  const onRefreshed = (token: string) => { subscribers.forEach(cb => cb(token)); subscribers.length = 0 }
  // 请求拦截器
  instance.interceptors.request.use(
    // 发送之前
    config => config,
    // 发送失败
    error => Promise.reject(error)
  )
  // 响应拦截器（可根据具体业务作出相应的调整）
  instance.interceptors.response.use(
    (response) => {
      return response.data
    },
    (error) => {
      const status = get(error, "response.status") as number | undefined
      const message = get(error, "response.data") || get(error, "message") || "请求错误"
      const originalConfig = (error?.config || {}) as AxiosRequestConfig & { _retry?: boolean }

      if (status === 401) {
        const reqUrl = String(get(error, "config.url") || "")
        // 避免刷新接口自身循环
        if (/refresh$/i.test(reqUrl)) {
          emitToast({ type: 'error', message: "登录已过期，请重新登录" })
          setTimeout(() => { logout() }, 300)
          return Promise.reject(error)
        }
        // 已重试仍失败则退出
        if (originalConfig._retry) {
          setTimeout(() => { logout() }, 0)
          return Promise.reject(error)
        }

        if (!isRefreshing) {
          isRefreshing = true
          refreshPromise = refreshTokenRequest()
            .then((data) => {
              const newToken = data?.access_token
              if (!newToken) throw new Error("刷新失败：未返回 access_token")
              setToken(newToken)
              isRefreshing = false
              onRefreshed(newToken)
              return newToken
            })
            .catch((e) => {
              isRefreshing = false
              emitToast({ type: 'error', message: "登录已过期，请重新登录" })
              setTimeout(() => { logout() }, 0)
              throw e
            })
        }

        return new Promise((resolve, reject) => {
          subscribeTokenRefresh((newToken) => {
            if (!originalConfig.headers) originalConfig.headers = {}
            originalConfig.headers["Authorization"] = `Bearer ${newToken}`
            originalConfig._retry = true
            resolve(instance(originalConfig))
          })
          refreshPromise?.catch(reject)
        })
      }

      // 其他状态码处理
      switch (status) {
        case 400:
          error.message = "请求错误"
          break
        case 403:
          error.message = message || "拒绝访问"
          break
        case 404:
          error.message = "请求地址出错"
          break
        case 408:
          error.message = "请求超时"
          break
        case 409:
          error.message = message || "已存在"
          break
        case 500:
          error.message = "服务器内部错误"
          break
        case 501:
          error.message = "服务未实现"
          break
        case 502:
          error.message = "网关错误"
          break
        case 503:
          error.message = "服务不可用"
          break
        case 504:
          error.message = "网关超时"
          break
        case 505:
          error.message = "HTTP 版本不受支持"
          break
      }
      emitToast({ type: 'error', message: error.message || String(message) })
      return Promise.reject(error)
    }
  )
  return instance
}

/** 创建 json 请求方法 */
function createRequest(instance: AxiosInstance) {
  return <T>(config: AxiosRequestConfig): Promise<T> => {
    const token = getToken()
    // 默认配置
    const defaultConfig: AxiosRequestConfig = {
      // 接口地址
      baseURL: import.meta.env.VITE_BASE_URL,
      // 请求头
      headers: {
        // 携带 Token
        "Authorization": token ? `Bearer ${token}` : undefined,
        "Content-Type": "application/json"
      },
      // 请求体
      data: {},
      // 请求超时
      timeout: 8000,
      // 跨域请求时是否携带 Cookies
      withCredentials: true
    }
    // 将默认配置 defaultConfig 和传入的自定义配置 config 进行合并成为 mergeConfig
    const mergeConfig = merge(defaultConfig, config)
    return instance(mergeConfig)
  }
}

/** 用于请求的实例 */
const instance = createInstance()

/** 用于请求的方法 */
export const request = createRequest(instance)
