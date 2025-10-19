// 统一处理 Cookie

import { CacheKey } from "@/common/constants/cache-key"
import Cookies from "js-cookie"

// 处理 token
export function getToken() {
  return Cookies.get(CacheKey.TOKEN)
}

export function setToken(token: string) {
  Cookies.set(CacheKey.TOKEN, token)
}

export function removeToken() {
  Cookies.remove(CacheKey.TOKEN)
}
