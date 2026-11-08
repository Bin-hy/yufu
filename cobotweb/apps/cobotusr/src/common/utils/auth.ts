import type { Router } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import { pinia } from '@/pinia'
import { useTabsStore } from '@/pinia/stores/tabs'
import { checkDrafts } from '@/workspace/draft-registry'
import { logoutRequest } from '@/pages/login/api'
export async function logoutAndRedirect(router: Router): Promise<void> {
    if (
        !(await checkDrafts(
            useTabsStore(pinia).visited.map((tab) => tab.key),
            true,
        ))
    )
        return
    const request = logoutRequest().catch(() => undefined)
    useUserStore(pinia).logout()
    await router.replace('/login')
    await request
}
export async function logoutOnly(router?: Router): Promise<void> {
    const request = logoutRequest().catch(() => undefined)
    useUserStore(pinia).logout()
    if (router) await router.replace('/login')
    await request
}
