import { computed, ref, watch } from 'vue'
import { usePageContext } from '@/workspace/page-context'
import { useAsyncQuery } from './use-async-query'
import type { Page } from '@/pages/account/api/type'

/**
 * Cursor pagination that accumulates pages ("load more" appends instead of
 * replacing). Loading one more page reuses the already fetched items; a
 * refresh after a mutation re-reads every loaded page so earlier rows cannot
 * go stale. Changing the base key (e.g. the search text) resets to page one.
 */
export function useCursorList<T, C extends string | number = string>(
    key: string | (() => string),
    loader: (after: C | undefined, signal: AbortSignal) => Promise<Page<T, C>>,
    active?: () => boolean,
) {
    const context = usePageContext()
    const baseKey = () => (typeof key === 'function' ? key() : key)
    const depth = ref(0)
    let cache: { base: string; depth: number; items: T[]; cursor: C } | null = null
    let reload = false
    const state = useAsyncQuery<Page<T, C>>({
        key: () => `${baseKey()}#${depth.value}`,
        preserveKey: baseKey,
        active: () => (active ? active() : context.active.value),
        loader: async (signal) => {
            const base = baseKey(),
                target = depth.value
            let items: T[] = []
            let after: C | undefined
            let start = 0
            const resume = !reload && cache && cache.base === base && cache.depth === target - 1
            if (resume && cache) {
                items = cache.items
                after = cache.cursor
                start = target
            }
            reload = false
            let next: C | null = null
            for (let index = start; index <= target; index++) {
                const page = await loader(after, signal)
                items = items.concat(page.items)
                next = page.next
                if (next == null) break
                after = next
            }
            cache = next == null ? null : { base, depth: target, items, cursor: next }
            return { items, next }
        },
    })
    watch(baseKey, (value, previous) => {
        if (value !== previous) {
            depth.value = 0
            cache = null
        }
    })
    const items = computed(() => state.data.value?.items ?? [])
    const hasMore = computed(() => state.data.value?.next != null)
    return {
        ...state,
        items,
        hasMore,
        next: () => {
            if (state.data.value?.next != null && !state.refreshing.value) depth.value++
        },
        first: () => {
            cache = null
            reload = true
            if (depth.value === 0) state.invalidate()
            else depth.value = 0
        },
        refresh: async () => {
            reload = true
            await state.refresh()
        },
    }
}
