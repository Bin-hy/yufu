import type { Router } from 'vue-router'
import { getToken } from '@/common/cache/cookies'
import { pinia } from '@/pinia'
import { useTabsStore } from '@/pinia/stores/tabs'
import { resolveTabIdentity } from '@/workspace/tab-identity'
import { checkDrafts } from '@/workspace/draft-registry'
import { checkedCloseKeys, noteNavigation } from '@/workspace/coordinator'
import { updatePageRoute } from '@/workspace/page-registry'
import { hasUnsavedWork } from '@/workspace/draft-registry'
import { saveWorkspace } from '@/storage/workspace'
import { sessionEpoch, sessionOwner } from '@/workspace/session'
import { confirmationBusy } from '@/utils/modal'
let registered = false
export function RegistryRouterGuard(router: Router) {
    if (registered) return
    registered = true
    router.beforeEach(async (to, from) => {
        if (to.meta.requiresAuth !== false && !getToken())
            return to.path.startsWith('/oidc/')
                ? { name: 'oidc-login', query: to.query, replace: true }
                : { name: 'login', query: { redirect: to.fullPath }, replace: true }
        if (confirmationBusy.value) return false
        const fromKey = from.meta.workspace ? resolveTabIdentity(from) : String(from.name ?? '')
        const toKey = to.meta.workspace ? resolveTabIdentity(to) : String(to.name ?? '')
        if (fromKey && !checkedCloseKeys.has(fromKey) && fromKey !== toKey) {
            const epoch = sessionEpoch.value
            if (!(await checkDrafts([fromKey])) || epoch !== sessionEpoch.value) return false
        }
    })
    router.afterEach((to, _from, failure) => {
        if (failure) return
        noteNavigation()
        if (to.meta.workspace) {
            const tabs = useTabsStore(pinia)
            tabs.sync(to)
            updatePageRoute(tabs.activeKey, to)
            saveWorkspace(router, sessionOwner.value, tabs.visited, tabs.activeKey)
        }
        document.title = String(to.meta.title ?? '工作台') + ' · cobotusr'
    })
    window.addEventListener('beforeunload', (event) => {
        if (hasUnsavedWork.value || confirmationBusy.value) {
            event.preventDefault()
            event.returnValue = ''
        }
    })
}
