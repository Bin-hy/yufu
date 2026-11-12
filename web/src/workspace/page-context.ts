import { computed, inject, type InjectionKey } from 'vue'
import { useRoute } from 'vue-router'
import type { PageContext } from '@/types/workspace'
import { sessionEpoch } from './session'
export const pageContextKey: InjectionKey<PageContext> = Symbol('workspace-page')
export function usePageContext(): PageContext {
    const context = inject(pageContextKey, null)
    if (context) return context
    const route = useRoute()
    return {
        key: String(route.name ?? 'standalone'),
        route: computed(() => route),
        active: computed(() => true),
        sessionEpoch,
    }
}
