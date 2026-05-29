import { createRequest } from '@cobotweb/shared/http'

// cobotlet 端 HTTP 直连本机 cobotlet 服务（无鉴权）。
// 通过 vite proxy 转发；生产环境直接同源。
export const request = createRequest({
  baseURL: import.meta.env.VITE_BASE_URL ?? '',
  getToken: () => undefined,
  onError: (msg) => console.warn('[cobotlet]', msg),
})
