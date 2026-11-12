import type { Router } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
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
export function canEnter(meta: { qualification?: 'developer' | 'administrator' }, identity: { developer_enabled: boolean; administrator: boolean } | null) {
    if (!identity) return false
    if (meta.qualification === 'administrator') return identity.administrator
    if (meta.qualification === 'developer') return identity.developer_enabled
    return true
}
export function RegistryRouterGuard(router: Router) {
    if (registered) return
    registered = true
    router.beforeEach(async (to, from) => {
        if (to.meta.requiresAuth !== false) {
            const user = useUserStore(pinia)
            await user.ensureSession()
            if (!user.centerSession || user.identityError)
                return { name: 'login', query: { redirect: to.fullPath }, replace: true }
            if (!canEnter(to.meta, user.centerSession))
                return { name: '403', replace: true }
        }
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
        document.title = String(to.meta.title ?? '工作台') + ' · yufu'
    })
    window.addEventListener('beforeunload', (event) => {
        if (hasUnsavedWork.value || confirmationBusy.value) {
            event.preventDefault()
            event.returnValue = ''
        }
    })
}
