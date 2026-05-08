import { CacheKey } from '@/common/constants/cache-key'

// #region 记住用户
export function setRememberUser(isRememberUser: string) {
  localStorage.setItem(CacheKey.REMEMBER_USER, isRememberUser.toString())
}

export function getRememberUser() {
  return localStorage.getItem(CacheKey.REMEMBER_USER)
}

export function removeRememberUser() {
  localStorage.removeItem(CacheKey.REMEMBER_USER)
}
// #endregion
