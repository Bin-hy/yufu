import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, ref, nextTick } from 'vue'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { useCursorList } from '@/composables/use-cursor-list'
import { resetSession } from '@/workspace/session'
import type { Page } from '@/pages/account/api/type'

let wrappers: VueWrapper[] = []
beforeEach(() => {
    resetSession()
})
afterEach(() => {
    wrappers.forEach((w) => w.unmount())
    wrappers = []
})

function pages(data: Record<string, Page<number, number>>) {
    const loader = vi.fn(async (after: number | undefined) => {
        const page = data[String(after ?? 'first')]
        if (!page) throw new Error('no page for cursor ' + after)
        return page
    })
    return loader
}
async function harness(loader: (after: number | undefined, signal: AbortSignal) => Promise<Page<number, number>>) {
    const base = ref('base')
    const exposed: { list?: ReturnType<typeof useCursorList<number, number>> } = {}
    const component = defineComponent({
        setup() {
            exposed.list = useCursorList<number, number>(() => base.value, loader)
            return () => h('div')
        },
    })
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/', name: 'home', component: { render: () => null } }],
    })
    await router.push('/')
    const wrapper = mount(component, { global: { plugins: [router] } })
    wrappers.push(wrapper)
    await flushPromises()
    return { list: exposed.list!, base }
}

it('加载更多追加而不是替换，且只请求下一页', async () => {
    const loader = pages({
        first: { items: [1, 2], next: 2 },
        '2': { items: [3, 4], next: 4 },
        '4': { items: [5], next: null },
    })
    const { list } = await harness(loader)
    expect(list.items.value).toEqual([1, 2])
    expect(list.hasMore.value).toBe(true)
    list.next()
    await flushPromises()
    expect(list.items.value).toEqual([1, 2, 3, 4])
    expect(loader).toHaveBeenCalledTimes(2)
    expect(loader.mock.calls[1]![0]).toBe(2)
    list.next()
    await flushPromises()
    expect(list.items.value).toEqual([1, 2, 3, 4, 5])
    expect(list.hasMore.value).toBe(false)
})
it('写操作后的刷新重读全部已加载页，保留展示且不丢前页更新', async () => {
    const loader = pages({
        first: { items: [1, 2], next: 2 },
        '2': { items: [3], next: null },
    })
    const { list } = await harness(loader)
    list.next()
    await flushPromises()
    expect(list.items.value).toEqual([1, 2, 3])
    loader.mockClear()
    // The first page changed server-side; a refresh must see it.
    loader.mockImplementation(async (after: number | undefined) =>
        after === undefined ? { items: [9, 2], next: 2 } : { items: [3], next: null },
    )
    await list.refresh()
    expect(list.items.value).toEqual([9, 2, 3])
    expect(loader).toHaveBeenCalledTimes(2)
    expect(loader.mock.calls.map((call) => call[0])).toEqual([undefined, 2])
})
it('基础键变化（如搜索词）回到第一页并清空累积', async () => {
    const loader = pages({
        first: { items: [1, 2], next: 2 },
        '2': { items: [3], next: null },
    })
    const { list, base } = await harness(loader)
    list.next()
    await flushPromises()
    expect(list.items.value).toEqual([1, 2, 3])
    loader.mockImplementation(async () => ({ items: [7], next: null }))
    base.value = 'base:query'
    await nextTick()
    await flushPromises()
    expect(list.items.value).toEqual([7])
    expect(list.hasMore.value).toBe(false)
})
