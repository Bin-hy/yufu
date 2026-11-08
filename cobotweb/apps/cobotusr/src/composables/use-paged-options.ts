import {
    ref,
    shallowRef,
    watch,
    toValue,
    onScopeDispose,
    getCurrentInstance,
    type MaybeRefOrGetter,
} from 'vue'
import type { PageResult } from '@/types/pagination'
import { errorMessage } from './use-async-mutation'
import { sessionEpoch, registerRead } from '@/workspace/session'
import { usePageContext } from '@/workspace/page-context'
export function usePagedOptions<T extends { id: number }>(
    loader: (page: number, size: number, signal: AbortSignal) => Promise<PageResult<T>>,
    source: MaybeRefOrGetter<string> = '',
    options: { active?: MaybeRefOrGetter<boolean> } = {},
) {
    const context = getCurrentInstance() ? usePageContext() : null
    const active = () => toValue(options.active ?? context?.active ?? true)
    const items = shallowRef<T[]>([]),
        loading = ref(false),
        error = ref<string | null>(null),
        hasMore = ref(true)
    let page = 1,
        generation = 0,
        controller: AbortController | null = null,
        needsLoad = true
    function cancel() {
        generation++
        controller?.abort()
        controller = null
        loading.value = false
    }
    async function loadNext() {
        if (!active() || loading.value || !hasMore.value) return
        const id = generation,
            epoch = sessionEpoch.value
        loading.value = true
        error.value = null
        needsLoad = true
        controller = new AbortController()
        const signal = controller.signal
        const unregister = registerRead(controller)
        try {
            const result = await loader(page, 100, signal)
            if (id !== generation || epoch !== sessionEpoch.value || signal.aborted) return
            const map = new Map(items.value.map((item) => [item.id, item]))
            result.data_list.forEach((item) => map.set(item.id, item))
            items.value = [...map.values()]
            hasMore.value = page < result.total_pages
            page++
            needsLoad = false
        } catch (e) {
            if (id === generation && !signal.aborted && epoch === sessionEpoch.value) {
                error.value = errorMessage(e)
                needsLoad = false
            }
        } finally {
            unregister()
            if (id === generation && epoch === sessionEpoch.value) {
                loading.value = false
                controller = null
            }
        }
    }
    function reset() {
        cancel()
        items.value = []
        page = 1
        hasMore.value = true
        error.value = null
        needsLoad = true
    }
    watch(
        () => [toValue(source), active(), sessionEpoch.value] as const,
        ([key, enabled, epoch], old) => {
            if (!old || old[0] !== key || old[2] !== epoch) reset()
            if (!enabled) {
                cancel()
                return
            }
            if (needsLoad) void loadNext()
        },
        { immediate: true },
    )
    onScopeDispose(reset)
    return { items, loading, error, hasMore, loadNext, retry: loadNext, reset }
}
export async function readAllPages<T>(
    loader: (page: number, size: number, signal: AbortSignal) => Promise<PageResult<T>>,
    signal: AbortSignal,
): Promise<T[]> {
    const result: T[] = []
    let page = 1,
        total = 1
    do {
        if (signal.aborted) throw new DOMException('Aborted', 'AbortError')
        const next = await loader(page, 100, signal)
        if (
            !Array.isArray(next.data_list) ||
            !Number.isSafeInteger(next.total_pages) ||
            next.total_pages < 0
        )
            throw new Error('分页响应不完整，请重新获取')
        result.push(...next.data_list)
        total = next.total_pages
        page++
    } while (page <= total)
    return result
}
