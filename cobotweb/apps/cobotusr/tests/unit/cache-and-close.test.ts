import { beforeEach, describe, expect, it } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory, type Router } from 'vue-router'
import { defineComponent, h, ref, onUnmounted, watch, nextTick } from 'vue'
import { pinia } from '@/pinia'
import { useTabsStore } from '@/pinia/stores/tabs'
import AppMain from '@/layouts/components/AppMain/index.vue'
import { usePageContext } from '@/workspace/page-context'
import { resetPageRegistry, updatePageRoute } from '@/workspace/page-registry'
import { closeWorkspaceTab, noteNavigation } from '@/workspace/coordinator'
import { useFormDraft } from '@/composables/use-form-draft'
import { currentConfirmation, settleConfirmation } from '@/utils/modal'
import { resetSession } from '@/workspace/session'
const tabs = useTabsStore(pinia)
beforeEach(() => {
    resetSession()
    tabs.reset()
    resetPageRegistry()
})
async function fixture() {
    const unmounted: string[] = [],
        changes = new Map<string, string[]>(),
        drafts = new Map<string, ReturnType<typeof useFormDraft>>()
    const Leaf = defineComponent({
        setup() {
            const context = usePageContext(),
                value = ref('')
            drafts.set(
                context.key,
                useFormDraft('对象编辑', () => value.value),
            )
            changes.set(context.key, [])
            watch(
                () => String(context.route.value.query.q ?? ''),
                (value) => changes.get(context.key)!.push(value),
                { immediate: true },
            )
            onUnmounted(() => unmounted.push(context.key))
            return () =>
                h('input', {
                    value: value.value,
                    'data-key': context.key,
                    onInput: (e: Event) => {
                        value.value = (e.target as HTMLInputElement).value
                    },
                })
        },
    })
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            {
                path: '/dashboard',
                name: 'dashboard-home',
                component: Leaf,
                meta: { workspace: true, affix: true },
            },
            {
                path: '/objects/:id',
                name: 'object',
                component: Leaf,
                meta: { workspace: true, tabIdentity: 'params', tabParamKeys: ['id'] },
            },
        ],
    })
    router.afterEach((to, _from, failure) => {
        if (!failure) {
            noteNavigation()
            tabs.sync(to)
            updatePageRoute(tabs.activeKey, to)
        }
    })
    await router.push('/objects/1?q=one')
    await router.isReady()
    const wrapper = mount(AppMain, {
        global: { plugins: [pinia, router] },
        attachTo: document.body,
    })
    await flushPromises()
    return { router, wrapper, unmounted, changes, drafts }
}
async function enter(router: Router, path: string) {
    await router.push(path)
    await flushPromises()
}
describe('公开 KeepAlive 与真实路由的集成', () => {
    it('同组件两个对象分别保留输入和query，关闭一个真实卸载且不影响另一个', async () => {
        const { router, wrapper, changes, unmounted, drafts } = await fixture(),
            first = tabs.activeKey
        await wrapper.get('input').setValue('first draft')
        drafts.get(first)!.markSaved()
        await enter(router, '/objects/2?q=two')
        const second = tabs.activeKey
        await wrapper.get('input').setValue('second draft')
        drafts.get(second)!.markSaved()
        await enter(router, '/objects/2?q=new-two')
        expect(changes.get(first)).toEqual(['one'])
        expect(changes.get(second)).toEqual(['two', 'new-two'])
        await enter(router, '/objects/1?q=one')
        expect(wrapper.get('input').element.value).toBe('first draft')
        await enter(router, '/objects/2?q=new-two')
        expect(await closeWorkspaceTab(router, first)).toBe(true)
        expect(unmounted).toContain(first)
        expect(unmounted).not.toContain(second)
        expect(wrapper.get('input').element.value).toBe('second draft')
        await enter(router, '/objects/1?q=one')
        expect(wrapper.get('input').element.value).toBe('')
        wrapper.unmount()
    })
    it('活动标签回退导航被守卫拒绝时，标签、缓存和输入均保留', async () => {
        const { router, wrapper, drafts, unmounted } = await fixture(),
            key = tabs.activeKey
        await wrapper.get('input').setValue('keep')
        drafts.get(key)!.markSaved()
        router.beforeEach((to) => (to.path === '/dashboard' ? false : undefined))
        expect(await closeWorkspaceTab(router, key)).toBe(false)
        expect(tabs.activeKey).toBe(key)
        expect(wrapper.get('input').element.value).toBe('keep')
        expect(unmounted).toEqual([])
        wrapper.unmount()
    })
    it('批量关闭包含脏表单时取消不删除任何页，确认后才销毁', async () => {
        const { router, wrapper } = await fixture()
        await wrapper.get('input').setValue('draft')
        const first = tabs.activeKey
        await enter(router, '/objects/2')
        const second = tabs.activeKey,
            original = tabs.visited.map((tab) => tab.key)
        const cancelled = closeWorkspaceTab(router, second, 'others')
        await nextTick()
        expect(currentConfirmation.value).not.toBeNull()
        settleConfirmation(false)
        expect(await cancelled).toBe(false)
        expect(tabs.visited.map((tab) => tab.key)).toEqual(original)
        const confirmed = closeWorkspaceTab(router, second, 'others')
        await nextTick()
        settleConfirmation(true)
        expect(await confirmed).toBe(true)
        expect(tabs.visited.map((tab) => tab.key)).toEqual(['dashboard-home', second])
        expect(tabs.visited.some((tab) => tab.key === first)).toBe(false)
        wrapper.unmount()
    })
    it('慢关闭确认期间新导航或重复关闭，不允许旧事务删除新页面', async () => {
        const { router, wrapper } = await fixture()
        await wrapper.get('input').setValue('draft')
        const key = tabs.activeKey
        const closing = closeWorkspaceTab(router, key)
        await nextTick()
        expect(await closeWorkspaceTab(router, key)).toBe(false)
        await enter(router, '/objects/2')
        settleConfirmation(true)
        expect(await closing).toBe(false)
        expect(tabs.visited.some((tab) => tab.key === key)).toBe(true)
        wrapper.unmount()
    })
    it('右侧关闭保留左侧与固定首页，活动回退完成后再清理', async () => {
        const { router, wrapper, unmounted } = await fixture(),
            first = tabs.activeKey
        await enter(router, '/objects/2')
        const second = tabs.activeKey
        await enter(router, '/objects/3')
        const third = tabs.activeKey
        expect(await closeWorkspaceTab(router, first, 'right')).toBe(true)
        expect(router.currentRoute.value.path).toBe('/objects/1')
        expect(tabs.visited.map((tab) => tab.key)).toEqual(['dashboard-home', first])
        expect(unmounted).toContain(second)
        expect(unmounted).toContain(third)
        wrapper.unmount()
    })
})
