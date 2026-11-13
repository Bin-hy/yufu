import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { effectScope, nextTick } from 'vue'
import { pinia } from '@/pinia'
import { useToastStore } from '@/pinia/stores/toast'
import FailureFeedback from '@/components/Feedback/failure-feedback.vue'
import ConfirmHost from '@/components/Feedback/confirm-host.vue'
import DataState from '@/components/workspace/data-state.vue'
import { normalizeFailure } from '@/http/errors'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useAsyncQuery } from '@/composables/use-async-query'
import { cleanupConfirmations } from '@/utils/modal'
import { resetSession } from '@/workspace/session'
const failed = (code: string) => ({ response: { status: 409, data: { code, request_id: 'req-ui' } } })
afterEach(() => { useToastStore(pinia).clear(); vi.useRealTimers() })
describe('持久错误和恢复操作', () => {
    it('没有回调不显示空操作，有回调时防止重复点击', async () => {
        let release!: () => void
        const recover = vi.fn(() => new Promise<void>(resolve => { release = resolve }))
        const wrapper = mount(FailureFeedback, { props: { failure: normalizeFailure(failed('conflict')), recoveryLabel: '核对最新结果' } })
        expect(wrapper.attributes('role')).toBe('alert')
        expect(wrapper.text()).toContain('req-ui')
        expect(wrapper.find('button').exists()).toBe(false)
        await wrapper.setProps({ recover })
        await wrapper.get('button').trigger('click')
        await wrapper.get('button').trigger('click')
        expect(recover).toHaveBeenCalledTimes(1)
        expect(wrapper.get('button').attributes('disabled')).toBeDefined()
        release(); await flushPromises()
        expect(wrapper.get('button').attributes('disabled')).toBeUndefined()
        wrapper.unmount()
    })
    it('数据加载失败可恢复，刷新失败保留旧结果', async () => {
        const loader = vi.fn().mockRejectedValueOnce(failed('forbidden')).mockResolvedValueOnce('旧结果').mockRejectedValueOnce(failed('dependency_unavailable')).mockResolvedValueOnce('新结果')
        const scope = effectScope()
        const query = scope.run(() => useAsyncQuery({ key: 'feedback-query', loader }))!
        await flushPromises()
        const wrapper = mount(DataState, { props: { status: query.status.value, error: query.error.value }, slots: { default: '旧结果' } })
        expect(wrapper.text()).toContain('加载失败')
        await query.refresh()
        expect(query.data.value).toBe('旧结果')
        await query.refresh()
        expect(query.data.value).toBe('旧结果')
        await wrapper.setProps({ status: query.status.value, error: query.error.value })
        expect(wrapper.text()).toContain('刷新失败，以下为上次结果')
        expect(wrapper.text()).toContain('旧结果')
        await query.refresh()
        expect(query.data.value).toBe('新结果'); expect(query.error.value).toBeNull()
        wrapper.unmount(); scope.stop()
    })
    it('取消解除忙碌且不显示错误，结果未知阻止再次写入', async () => {
        const mutation = useAsyncMutation()
        await mutation.run(async () => { throw { code: 'ERR_CANCELED' } })
        expect(mutation.error.value).toBeNull(); expect(mutation.pending.value).toBe(false)
        const write = vi.fn().mockRejectedValue(failed('commit_uncertain'))
        await mutation.run(write); await mutation.run(write)
        expect(write).toHaveBeenCalledTimes(1)
        expect(mutation.failure.value?.outcomeUncertain).toBe(true)
    })
    it('写入成功但后续处理失败保留已完成事实', async () => {
        const mutation = useAsyncMutation()
        expect(await mutation.run(async () => true, async () => { throw failed('dependency_unavailable') })).toBe(true)
        expect(mutation.error.value).toContain('操作已完成')
        expect(mutation.failure.value?.outcomeUncertain).toBe(false)
    })
    it('确认结果未知保留弹窗且禁止重复执行，取消可结束流程', async () => {
        const host = mount(ConfirmHost, { attachTo: document.body })
        const mutation = useAsyncMutation()
        const write = vi.fn().mockRejectedValue(failed('commit_uncertain'))
        const result = mutation.confirm({ title: '停用对象', content: '撤销该对象的全部授权', destructive: true }, write)
        await flushPromises()
        const confirm = () => [...document.querySelectorAll<HTMLButtonElement>('button')].find(b => b.textContent === '确认')!
        confirm().click(); await flushPromises()
        expect(document.body.textContent).toContain('可能已经生效')
        expect(confirm().disabled).toBe(true)
        confirm().click(); expect(write).toHaveBeenCalledTimes(1)
        const cancel = [...document.querySelectorAll<HTMLButtonElement>('button')].find(b => b.textContent === '取消')!
        cancel.click(); await expect(result).resolves.toBe(false)
        await nextTick(); cleanupConfirmations(); host.unmount()
    })
})
describe('Toast 生命周期', () => {
    it('合并同原因不同请求标识，独立重试可再次提醒，数量有上限', () => {
        vi.useFakeTimers()
        const store = useToastStore(pinia)
        store.clear()
        store.push('error', '失败（请求标识 one）')
        store.push('error', '失败（请求标识 two）')
        expect(store.toasts).toHaveLength(1)
        expect(store.toasts[0]?.durationMs).toBe(6000)
        vi.advanceTimersByTime(3001)
        store.push('error', '失败（请求标识 three）')
        expect(store.toasts).toHaveLength(2)
        for (let i = 0; i < 8; i++) store.push('error', `原因 ${i}`)
        expect(store.toasts).toHaveLength(4)
        resetSession()
        expect(store.toasts).toHaveLength(0); expect(vi.getTimerCount()).toBe(0)
    })
    it('显式永久展示可关闭，默认警告六秒，普通成功三秒', () => {
        vi.useFakeTimers()
        const store = useToastStore(pinia); store.clear()
        const id = store.push('error', '重要错误', 0)
        store.warning('警告'); store.success('成功')
        vi.advanceTimersByTime(3000)
        expect(store.toasts.map(t => t.message)).toEqual(['重要错误', '警告'])
        vi.advanceTimersByTime(3000)
        expect(store.toasts.map(t => t.message)).toEqual(['重要错误'])
        store.remove(id); expect(store.toasts).toHaveLength(0)
    })
})

it('恢复操作失败显示原因并允许再次核对，不产生未处理异常', async () => {
    const recover = vi.fn().mockRejectedValue(failed('dependency_unavailable'))
    const wrapper = mount(FailureFeedback, { props: { failure: normalizeFailure(failed('commit_uncertain')), recoveryLabel: '核对结果', recover } })
    await wrapper.get('button').trigger('click'); await flushPromises()
    expect(wrapper.text()).toContain('核对失败')
    expect(wrapper.text()).toContain('认证服务暂时不可用')
    expect(wrapper.get('button').attributes('disabled')).toBeUndefined()
    wrapper.unmount()
})
