import { computed, defineComponent, h, markRaw, provide, shallowRef, type Component } from 'vue'
import type { RouteLocationNormalizedLoaded } from 'vue-router'
import type { PageContext, WorkspaceTab } from '@/types/workspace'
import { pageContextKey } from './page-context'
import { sessionEpoch, onSessionReset } from './session'
interface PageEntry {
    component: Component
    context: PageContext
    snapshot: ReturnType<typeof shallowRef<RouteLocationNormalizedLoaded>>
}
const entries = new Map<string, PageEntry>()
export function getPageComponent(
    tab: WorkspaceTab,
    leaf: Component,
    route: RouteLocationNormalizedLoaded,
    active: () => string,
): Component {
    const existing = entries.get(tab.key)
    if (existing) {
        existing.snapshot.value = route
        return existing.component
    }
    const snapshot = shallowRef(route)
    const context: PageContext = {
        key: tab.key,
        route: snapshot,
        active: computed(() => active() === tab.key),
        sessionEpoch,
    }
    const component = markRaw(
        defineComponent({
            name: tab.cacheName,
            setup() {
                provide(pageContextKey, context)
                return () => h(leaf, {})
            },
        }),
    )
    entries.set(tab.key, { component, context, snapshot })
    return component
}
export function updatePageRoute(key: string, route: RouteLocationNormalizedLoaded) {
    const entry = entries.get(key)
    if (entry) entry.snapshot.value = route
}
export function releasePage(key: string) {
    entries.delete(key)
}
export function resetPageRegistry() {
    entries.clear()
}
onSessionReset(resetPageRegistry)
