import { defineStore } from 'pinia'
import { ref } from 'vue'
import { setToken, removeToken } from '@/common/cache/cookies'
import { getCenterSession } from '@/pages/account/api'
import type { CenterSession } from '@/pages/account/api/type'
import { establishSession, sessionEpoch, resetSession, onSessionReset } from '@/workspace/session'
export interface UserInfo {
    id?: number
    username: string
    token?: string
    nickname: string | null
}
export const useUserStore = defineStore('user', () => {
    const userInfo = ref<UserInfo | null>(null),
        centerSession = ref<CenterSession | null>(null),
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
        centerSession.value = null
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
    function setCenterSession(info: CenterSession) {
        const previous = centerSession.value
        if (previous && (previous.sub !== info.sub || previous.developer_enabled !== info.developer_enabled
            || previous.administrator !== info.administrator || previous.sso_session_id !== info.sso_session_id)) resetSession()
        establishSession(info.user_id)
        centerSession.value = info
        userInfo.value = { id: info.user_id, username: info.username, nickname: info.nickname }
        isLoggedIn.value = true
        identityError.value = null
    }
    function initUserInfo() {
        // Stored labels/tokens are not evidence of a current center session.
        removeToken()
        userInfo.value = null
        isLoggedIn.value = false
    }
    async function ensureSession(): Promise<void> {
        if (restoring) return restoring
        const epoch = sessionEpoch.value
        loading.value = true
        identityError.value = null
        const current: Promise<void> = getCenterSession({ errorPresentation: 'local' })
            .then((info) => {
                if (epoch !== sessionEpoch.value) return
                setCenterSession(info)
            })
            .catch((error) => {
                if (epoch === sessionEpoch.value) {
                    if (error.response?.status === 401) resetSession()
                    identityError.value = error.message ?? '身份加载失败，请重试'
                }
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
        centerSession,
        isLoggedIn,
        loading,
        identityError,
        setUserInfo,
        setCenterSession,
        clearUserInfo: logout,
        logout,
        initUserInfo,
        ensureSession,
    }
})
