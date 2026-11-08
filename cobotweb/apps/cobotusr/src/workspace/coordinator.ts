import { nextTick } from 'vue'
import {
    isNavigationFailure,
    NavigationFailureType,
    type Router,
    type RouteLocationRaw,
} from 'vue-router'
import { useTabsStore } from '@/pinia/stores/tabs'
import { pinia } from '@/pinia'
import { checkDrafts, discardDrafts } from './draft-registry'
import { releasePage } from './page-registry'
import { sessionEpoch, resetSession, sessionOwner } from './session'
import { saveWorkspace } from '@/storage/workspace'
let navigationRevision = 0,
    closing = false
export const checkedCloseKeys = new Set<string>()
export function noteNavigation() {
    navigationRevision++
}
export async function navigateWorkspace(router: Router, to: RouteLocationRaw) {
    try {
        const result = await router.push(to)
        return !result
            ? 'success'
            : isNavigationFailure(result, NavigationFailureType.duplicated)
              ? 'unchanged'
              : 'cancelled'
    } catch {
        return 'failed'
    }
}
export async function closeWorkspaceTab(
    router: Router,
    key: string,
    scope: 'current' | 'others' | 'right' = 'current',
): Promise<boolean> {
    if (closing) return false
    const store = useTabsStore(pinia)
    const index = store.visited.findIndex((tab) => tab.key === key)
    if (index < 0) return false
    const targets = store.visited.filter(
        (tab, i) =>
            !tab.affix &&
            (scope === 'current'
                ? tab.key === key
                : scope === 'others'
                  ? tab.key !== key
                  : i > index),
    )
    if (!targets.length) return false
    const keys = targets.map((tab) => tab.key),
        epoch = sessionEpoch.value,
        revision = navigationRevision
    closing = true
    try {
        if (
            !(await checkDrafts(keys, true)) ||
            epoch !== sessionEpoch.value ||
            revision !== navigationRevision
        )
            return false
        const activeWillClose = keys.includes(store.activeKey)
        if (activeWillClose) {
            const activeIndex = store.visited.findIndex((tab) => tab.key === store.activeKey)
            const retained = store.visited.filter((tab) => !keys.includes(tab.key))
            const left = store.visited
                .slice(0, activeIndex)
                .reverse()
                .find((tab) => !keys.includes(tab.key))
            const next = left ?? retained[0]
            keys.forEach((key) => checkedCloseKeys.add(key))
            const outcome = await navigateWorkspace(router, next?.fullPath ?? '/dashboard')
            checkedCloseKeys.clear()
            if (outcome !== 'success' && outcome !== 'unchanged') return false
            if (
                epoch !== sessionEpoch.value ||
                navigationRevision !== revision + (outcome === 'success' ? 1 : 0) ||
                !retained.some((tab) => tab.key === store.activeKey)
            )
                return false
        }
        discardDrafts(keys)
        store.commitClose(keys)
        await nextTick()
        keys.forEach(releasePage)
        saveWorkspace(router, sessionOwner.value, store.visited, store.activeKey)
        return true
    } finally {
        closing = false
        checkedCloseKeys.clear()
    }
}
export { resetSession }
