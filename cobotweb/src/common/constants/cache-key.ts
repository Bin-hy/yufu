const SYSTEM_NAME = import.meta.env.VITE_SYSTEM_NAME || "serverfront-server-template"

/** 缓存数据时用到的 Key */
export class CacheKey {
    static readonly TOKEN = `${SYSTEM_NAME}-token-key` // token
    static readonly REMEMBER_USER = `${SYSTEM_NAME}-remember-user-key` // 记住我
}
