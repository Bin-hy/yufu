import { CacheKey } from '@/common/constants/cache-key'

// #region 记住用户
export function setRememberUser(isRememberUser: string) {
    try {
        localStorage.setItem(CacheKey.REMEMBER_USER, isRememberUser.toString())
    } catch {
        /* 记忆偏好不可用 */
    }
}

export function getRememberUser() {
    try {
        return localStorage.getItem(CacheKey.REMEMBER_USER)
    } catch {
        return null
    }
}

export function removeRememberUser() {
    try {
        localStorage.removeItem(CacheKey.REMEMBER_USER)
    } catch {
        /* 内存登录继续 */
    }
}
// #endregion
