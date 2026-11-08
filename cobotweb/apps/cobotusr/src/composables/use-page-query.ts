import { computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { usePageContext } from '@/workspace/page-context'
import type { PageQuery } from '@/types/workspace'
function positive(value: unknown, fallback: number) {
    const number = typeof value === 'string' && /^\d+$/.test(value) ? Number(value) : NaN
    return Number.isSafeInteger(number) && number > 0 ? number : fallback
}
export function normalizePageQuery(query: Record<string, unknown>): PageQuery {
    const size = positive(query.page_size, 10)
    const result: PageQuery = {
        page_number: positive(query.page_number, 1),
        page_size: [10, 20, 50].includes(size) ? size : 10,
    }
    if (typeof query.q === 'string' && query.q) result.q = query.q
    for (const key of ['roleId', 'application_id', 'user_id'] as const) {
        const number = positive(query[key], 0)
        if (number) result[key] = number
    }
    return result
}
export function usePageQuery() {
    const context = usePageContext(),
        router = useRouter()
    const query = computed(() => normalizePageQuery(context.route.value.query))
    async function update(values: Partial<PageQuery>, replace = false) {
        if (!context.active.value) return
        const next = { ...query.value, ...values }
        const cleaned = Object.fromEntries(
            Object.entries(next)
                .filter(([, value]) => value != null && value !== '')
                .map(([key, value]) => [key, String(value)]),
        )
        const target = { path: context.route.value.path, query: cleaned }
        if (router.resolve(target).fullPath === context.route.value.fullPath) return
        await router[replace ? 'replace' : 'push'](target)
    }
    const page = computed({
        get: () => query.value.page_number,
        set: (value) => {
            void update({ page_number: value })
        },
    })
    const size = computed({
        get: () => query.value.page_size,
        set: (value) => {
            void update({ page_size: value, page_number: 1 })
        },
    })
    const search = computed({
        get: () => query.value.q ?? '',
        set: (value) => {
            void update({ q: value, page_number: 1 }, true)
        },
    })
    watch(
        () => context.route.value.fullPath,
        () => {
            if (!context.active.value) return
            const raw = context.route.value.query
            if (
                ['page_number', 'page_size'].some(
                    (key) => raw[key] !== undefined && raw[key] !== String(query.value[key]),
                )
            )
                void update({}, true)
        },
        { immediate: true },
    )
    return {
        context,
        query,
        page,
        size,
        search,
        update,
        setPage: (page: number) => update({ page_number: page }),
        setSize: (size: number) => update({ page_size: size, page_number: 1 }),
        setFilters: (values: Partial<PageQuery>) => update({ ...values, page_number: 1 }),
        clamp: (total: number) => {
            if (page.value > Math.max(1, total))
                void update({ page_number: Math.max(1, total) }, true)
        },
    }
}
