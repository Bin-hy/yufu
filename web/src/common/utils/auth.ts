import type { Router } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import { pinia } from '@/pinia'
import { useTabsStore } from '@/pinia/stores/tabs'
import { checkDrafts } from '@/workspace/draft-registry'
import { centerLogout } from '@/pages/account/api'
import { emitToast } from '@/utils/toast'
async function endCenterSession() {
    try {
        await centerLogout()
        return true
    } catch (error) {
        if ((error as { response?: { status: number } }).response?.status === 401) return true
        emitToast({ type: 'error', message: (error as Error).message || '退出失败，请重试' })
        return false
    }
}
export async function logoutAndRedirect(router: Router): Promise<void> {
    if (
        !(await checkDrafts(
            useTabsStore(pinia).visited.map((tab) => tab.key),
            true,
        ))
    )
        return
    if (!(await endCenterSession())) return
    useUserStore(pinia).logout()
    await router.replace('/login')
}
export async function logoutOnly(router?: Router): Promise<void> {
    if (!(await endCenterSession())) return
    useUserStore(pinia).logout()
    if (router) await router.replace('/login')
}
