import { describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { effectScope, ref, nextTick } from 'vue'
import ConfirmHost from '@/components/Feedback/confirm-host.vue'
import {
    showModal,
    currentConfirmation,
    cleanupConfirmations,
    confirmationBusy,
} from '@/utils/modal'
import { resetSession } from '@/workspace/session'
import { usePagedOptions, readAllPages } from '@/composables/use-paged-options'
import { createVerifiedCreate } from '@/composables/use-verified-create'
function button(label: string) {
    return [...document.querySelectorAll<HTMLButtonElement>('button')].find(
        (b) => b.textContent?.trim() === label,
    )!
}
describe('确认宿主的结束与失败路径', () => {
    it('取消回调抛错仍兑现为 false', async () => {
        const host = mount(ConfirmHost, { attachTo: document.body }),
            result = showModal({
                title: 'cancel',
                content: 'cancel',
                cancleCallback: () => {
                    throw Error('callback failed')
                },
            })
        await flushPromises()
        button('取消').click()
        await expect(result).resolves.toBe(false)
        expect(currentConfirmation.value).toBeNull()
        host.unmount()
    })
    it('异步失败保持打开，重复确认只调用一次，重试成功兑现为 true', async () => {
        let reject!: (e: Error) => void
        const callback = vi
                .fn()
                .mockImplementationOnce(
                    () =>
                        new Promise((_res, rej) => {
                            reject = rej
                        }),
                )
                .mockResolvedValue(undefined),
            host = mount(ConfirmHost, { attachTo: document.body }),
            result = showModal({
                title: 'remove',
                content: 'remove',
                destructive: true,
                confirmCallback: callback,
            })
        await flushPromises()
        button('确认').click()
        button('确认')?.click()
        expect(callback).toHaveBeenCalledTimes(1)
        expect(confirmationBusy.value).toBe(true)
        reject(Error('service failed'))
        await flushPromises()
        expect(document.querySelector('[role="alert"]')?.textContent).toContain('操作失败，请稍后重试')
        button('确认').click()
        await expect(result).resolves.toBe(true)
        expect(confirmationBusy.value).toBe(false)
        host.unmount()
    })
    it('卸载、会话重置及显式清理结束所有排队请求', async () => {
        for (const stop of [() => cleanupConfirmations(), () => resetSession()]) {
            const host = mount(ConfirmHost),
                first = showModal({ title: 'a', content: 'a' }),
                second = showModal({ title: 'b', content: 'b' })
            stop()
            await expect(first).resolves.toBe(false)
            await expect(second).resolves.toBe(false)
            host.unmount()
        }
        const host = mount(ConfirmHost),
            result = showModal({ title: 'unmount', content: 'unmount' })
        host.unmount()
        await expect(result).resolves.toBe(false)
    })
})
describe('分页候选与完整基线', () => {
    it('失活不启动，加载更多去重，失败重试同一页', async () => {
        const active = ref(false),
            loader = vi
                .fn()
                .mockResolvedValueOnce({ total_pages: 3, data_list: [{ id: 1 }] })
                .mockRejectedValueOnce(Error('page failed'))
                .mockResolvedValueOnce({ total_pages: 3, data_list: [{ id: 1 }, { id: 2 }] })
                .mockResolvedValueOnce({ total_pages: 3, data_list: [{ id: 3 }] }),
            scope = effectScope(),
            options = scope.run(() => usePagedOptions(loader, '', { active }))!
        expect(loader).not.toHaveBeenCalled()
        active.value = true
        await flushPromises()
        await options.loadNext()
        expect(options.error.value).toContain('操作失败，请稍后重试')
        await options.retry()
        await options.loadNext()
        expect(options.items.value.map((row) => row.id)).toEqual([1, 2, 3])
        expect(loader.mock.calls.map((call) => call[0])).toEqual([1, 2, 2, 3])
        expect(options.hasMore.value).toBe(false)
        scope.stop()
    })
    it('失活取消请求，晚响应不覆盖新源；恢复激活可继续读取', async () => {
        let resolve!: (v: unknown) => void
        const active = ref(true),
            source = ref('a'),
            loader = vi
                .fn()
                .mockImplementationOnce(
                    () =>
                        new Promise((res) => {
                            resolve = res
                        }),
                )
                .mockResolvedValue({ total_pages: 1, data_list: [{ id: 2 }] }),
            scope = effectScope(),
            options = scope.run(() => usePagedOptions(loader, source, { active }))!
        active.value = false
        await nextTick()
        source.value = 'b'
        active.value = true
        await flushPromises()
        resolve({ total_pages: 1, data_list: [{ id: 1 }] })
        await flushPromises()
        expect(options.items.value).toEqual([{ id: 2 }])
        scope.stop()
    })
    it('读取 8105 条跨越旧上限，任意中间页失败拒绝半份基线', async () => {
        const data = Array.from({ length: 8105 }, (_, i) => i + 1),
            loader = vi.fn(async (page: number, size: number) => ({
                data_list: data.slice((page - 1) * size, page * size),
                total_pages: 82,
                page_number: page,
                page_size: size,
            }))
        expect(await readAllPages(loader, new AbortController().signal)).toEqual(data)
        expect(loader).toHaveBeenCalledTimes(82)
        await expect(
            readAllPages(async (page, size) => {
                if (page === 2) throw Error('page two')
                return { total_pages: 82, data_list: [page], page_number: page, page_size: size }
            }, new AbortController().signal),
        ).rejects.toThrow('page two')
    })
})
describe('非幂等创建结果核对', () => {
    it('超时但服务端已完成，通过读取恢复结果，不重发创建', async () => {
        const write = vi.fn().mockRejectedValue({ code: 'ECONNABORTED' }),
            find = vi.fn().mockResolvedValue({ id: 7 }),
            step = createVerifiedCreate(write, find)
        expect(await step.run()).toEqual({ id: 7 })
        expect(write).toHaveBeenCalledTimes(1)
        expect(find).toHaveBeenCalledTimes(1)
    })
    it('未知且无法核对，只重试读取，禁止再次 POST', async () => {
        const write = vi.fn().mockRejectedValue({ code: 'ERR_NETWORK' }),
            find = vi.fn().mockResolvedValue(null),
            step = createVerifiedCreate(write, find)
        await expect(step.run()).rejects.toThrow('创建结果未知')
        await expect(step.run()).rejects.toThrow('创建结果未知')
        expect(write).toHaveBeenCalledTimes(1)
        expect(find).toHaveBeenCalledTimes(2)
        find.mockResolvedValue({ id: 8 })
        expect(await step.run()).toEqual({ id: 8 })
    })
    it('已知 HTTP 失败可重试；取消与普通异常不冒充提交结果未知', async () => {
        const write = vi
                .fn()
                .mockRejectedValueOnce({ response: { status: 503 } })
                .mockResolvedValue({ id: 1 }),
            find = vi.fn(),
            step = createVerifiedCreate(write, find)
        await expect(step.run()).rejects.toEqual({ response: { status: 503 } })
        expect(await step.run()).toEqual({ id: 1 })
        expect(find).not.toHaveBeenCalled()
        await expect(
            createVerifiedCreate(async () => {
                throw { code: 'ERR_CANCELED' }
            }, find).run(),
        ).rejects.toEqual({ code: 'ERR_CANCELED' })
    })
})

it('服务端提交结果未知只核对读取，并保留原请求标识', async () => {
    const { normalizeFailure } = await import('@/http/errors')
    const write = vi.fn().mockRejectedValue({ response: { status: 503, data: { code: 'commit_uncertain', request_id: 'req-uncertain' } } })
    const find = vi.fn().mockResolvedValue(null)
    const flow = createVerifiedCreate(write, find)
    const error = await flow.run().catch(error => error)
    expect(normalizeFailure(error).requestId).toBe('req-uncertain')
    await expect(flow.run()).rejects.toThrow('创建结果未知')
    expect(write).toHaveBeenCalledTimes(1)
    expect(find).toHaveBeenCalledTimes(2)
})
