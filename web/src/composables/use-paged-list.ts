import { computed, watch } from 'vue'
import type { PageResult } from '@/types/pagination'
import { usePageQuery } from './use-page-query'
import { useAsyncQuery } from './use-async-query'
import { useTabsStore } from '@/pinia/stores/tabs'
import type { PageQuery } from '@/types/workspace'
export function usePagedList<T>(
    loader: (query: PageQuery, signal: AbortSignal) => Promise<PageResult<T>>,
) {
    const pager = usePageQuery(),
        tabs = useTabsStore()
    const state = useAsyncQuery({
        key: () =>
            JSON.stringify([
                pager.query.value.page_number,
                pager.query.value.page_size,
                pager.query.value.roleId,
                pager.query.value.application_id,
                pager.query.value.user_id,
            ]),
        active: pager.context.active,
        loader: (signal) => loader(pager.query.value, signal),
    })
    const rows = computed(() => state.data.value?.data_list ?? [])
    const totalPages = computed(() => state.data.value?.total_pages ?? 1)
    watch(totalPages, (total) => {
        if (state.status.value === 'ready') pager.clamp(total)
    })
    watch(
        () => tabs.invalidations[String(pager.context.route.value.name)] ?? 0,
        () => state.invalidate(),
    )
    return { ...pager, ...state, rows, totalPages }
}
