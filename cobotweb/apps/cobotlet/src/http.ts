import { createRequest } from '@cobotweb/shared/http'

// cobotlet 端 HTTP 直连本机 cobotlet 服务（无鉴权）。
// baseURL 必须留空走同源：webui 嵌入模式下页面由 cobotlet 自身托管，
// 请求打到当前端口（如 http://localhost:3000/devices）；dev 模式下由
// vite server 的 proxy 转发 /devices /tasks /hello 到 API_TARGET。
// 不要在这里注入 VITE_BASE_URL —— 它只应作为 vite.config.ts 的 proxy
// 目标，注入到 bundle 会把生产请求打向错误的固定端口（如 3080）。
export const request = createRequest({
  baseURL: '',
  getToken: () => undefined,
  onError: (msg) => console.warn('[cobotlet]', msg),
})
