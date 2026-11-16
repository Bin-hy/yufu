import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { usePageContext } from '@/workspace/page-context'

/**
 * Mirrors a small set of string filters into the route query so reloading or
 * sharing a link restores the view. Values equal to their default (or empty)
 * are removed from the URL; unrelated query parameters are preserved.
 */
export function useRouteFilters<K extends string>(defaults: Record<K, string>) {
    const context = usePageContext(),
        router = useRouter()
    const values = computed(() => {
        const query = context.route.value.query
        return Object.fromEntries(
            (Object.entries(defaults) as [K, string][]).map(([key, fallback]) => {
                const raw = query[key]
                return [key, typeof raw === 'string' && raw !== '' ? raw : fallback]
            }),
        ) as Record<K, string>
    })
    async function set(patch: Partial<Record<K, string>>) {
        if (!context.active.value) return
        const query: Record<string, unknown> = {}
        for (const [key, value] of Object.entries(context.route.value.query))
            if (!(key in defaults)) query[key] = value
        for (const [key, fallback] of Object.entries(defaults) as [K, string][]) {
            const value = patch[key] ?? values.value[key]
            if (value !== fallback && value !== '') query[key] = value
        }
        const target = { path: context.route.value.path, query: query as Record<string, string> }
        if (router.resolve(target).fullPath === context.route.value.fullPath) return
        await router.replace(target)
    }
    function model(key: K) {
        return computed({
            get: () => values.value[key],
            set: (value: string) => {
                void set({ [key]: value } as Partial<Record<K, string>>)
            },
        })
    }
    return { values, set, model }
}
