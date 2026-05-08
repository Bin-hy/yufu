// 统一处理 Cookie

import { CacheKey } from "@/common/constants/cache-key"
import Cookies from "js-cookie"

// 处理 token
export function getToken() {
  return Cookies.get(CacheKey.ACCESS_TOKEN)
}

export function setToken(token: string) {
  Cookies.set(CacheKey.ACCESS_TOKEN, token)
}

export function removeToken() {
  Cookies.remove(CacheKey.ACCESS_TOKEN)
}
