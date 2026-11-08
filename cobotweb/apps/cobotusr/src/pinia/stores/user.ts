import { defineStore } from 'pinia'
import { ref } from 'vue'
import { getToken, setToken, removeToken } from '@/common/cache/cookies'
import { getCurrentUser } from '@/pages/profile/api'
import { establishSession, sessionEpoch, resetSession, onSessionReset } from '@/workspace/session'
export interface UserInfo {
    id?: number
    username: string
    token?: string
    nickname: string | null
}
export const useUserStore = defineStore('user', () => {
    const userInfo = ref<UserInfo | null>(null),
        isLoggedIn = ref(false),
        loading = ref(false),
        identityError = ref<string | null>(null)
    let restoring: Promise<void> | null = null
    function persist(info: UserInfo) {
        try {
            localStorage.setItem(
                'userInfo',
                JSON.stringify({ username: info.username, nickname: info.nickname }),
            )
        } catch {
            /* 内存继续 */
        }
    }
    function setUserInfo(info: UserInfo) {
        userInfo.value = info
        isLoggedIn.value = true
        if (info.token) setToken(info.token)
        persist(info)
    }
    function clearLocal() {
        restoring = null
        loading.value = false
        identityError.value = null
        userInfo.value = null
        isLoggedIn.value = false
        removeToken()
        try {
            localStorage.removeItem('userInfo')
        } catch {
            /* 内存继续 */
        }
    }
    function logout() {
        resetSession()
        clearLocal()
    }
    onSessionReset(clearLocal)
    function initUserInfo() {
        if (!getToken()) return
        isLoggedIn.value = true
        try {
            const info = JSON.parse(localStorage.getItem('userInfo') ?? 'null')
            if (info?.username)
                userInfo.value = { username: info.username, nickname: info.nickname }
        } catch {
            /* 当前身份接口恢复 */
        }
    }
    async function ensureSession(): Promise<void> {
        if (restoring) return restoring
        if (!getToken()) return
        const epoch = sessionEpoch.value
        loading.value = true
        identityError.value = null
        const current: Promise<void> = getCurrentUser({ errorPresentation: 'local' })
            .then((info) => {
                if (epoch !== sessionEpoch.value) return
                establishSession(info.id)
                userInfo.value = { id: info.id, username: info.username, nickname: info.nickname }
                isLoggedIn.value = true
                persist(userInfo.value)
            })
            .catch((error) => {
                if (epoch === sessionEpoch.value)
                    identityError.value = error.message ?? '身份加载失败，请重试'
            })
            .finally(() => {
                if (restoring === current) restoring = null
                if (epoch === sessionEpoch.value) loading.value = false
            })
        restoring = current
        return current
    }
    return {
        userInfo,
        isLoggedIn,
        loading,
        identityError,
        setUserInfo,
        clearUserInfo: logout,
        logout,
        initUserInfo,
        ensureSession,
    }
})
