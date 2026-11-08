import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createRouter, createMemoryHistory } from 'vue-router'
import { createPinia, setActivePinia } from 'pinia'
import { effectScope, ref, nextTick } from 'vue'
import { useTabsStore } from '@/pinia/stores/tabs'
import { resolveTabIdentity } from '@/workspace/tab-identity'
import { encodeSnapshot, loadWorkspace, saveWorkspace } from '@/storage/workspace'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { readAllPages } from '@/composables/use-paged-options'
import { normalizePageQuery } from '@/composables/use-page-query'
import { resetSession, establishSession, sessionOwner } from '@/workspace/session'
import { cleanupConfirmations, showModal, settleConfirmation } from '@/utils/modal'
const router = createRouter({
    history: createMemoryHistory(),
    routes: [
        {
            path: '/dashboard',
            name: 'dashboard-home',
            component: {},
            meta: { workspace: true, affix: true },
        },
        {
            path: '/users',
            name: 'users',
            component: {},
            meta: { workspace: true, persistQueryKeys: ['page_number', 'q'] },
        },
        {
            path: '/users/:id',
            name: 'detail',
            component: {},
            meta: { workspace: true, tabIdentity: 'params', tabParamKeys: ['id'] },
        },
        { path: '/login', name: 'login', component: {}, meta: {} },
    ],
})
function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (reason: unknown) => void
    const promise = new Promise<T>((res, rej) => {
        resolve = res
        reject = rej
    })
    return { promise, resolve, reject }
}
beforeEach(() => {
    setActivePinia(createPinia())
    resetSession()
})
describe('标签与持久化', () => {
    it('query 不产生新标签，参数决定详情标签身份', () => {
        expect(resolveTabIdentity(router.resolve('/users?page_number=2'))).toBe('users')
        expect(resolveTabIdentity(router.resolve('/users/2'))).not.toBe(
            resolveTabIdentity(router.resolve('/users/3')),
        )
    })
    it('更新地址保留 cacheName，首页唯一且不可被关闭', () => {
        const tabs = useTabsStore()
        tabs.sync(router.resolve('/users?page_number=2'))
        const name = tabs.visited[1]!.cacheName
        tabs.sync(router.resolve('/users?page_number=3'))
        tabs.initHome()
        expect(tabs.visited).toHaveLength(2)
        expect(tabs.visited[1]!.cacheName).toBe(name)
        expect(tabs.activePath).toContain('page_number=3')
        tabs.commitClose(['dashboard-home'])
        expect(tabs.visited[0]!.affix).toBe(true)
    })
    it('快照剔除密钥、认证路由和 hash，owner 隔离', () => {
        const tabs = useTabsStore()
        tabs.sync(router.resolve('/users?page_number=3&secret=x#token'))
        const snapshot = encodeSnapshot(router, '7', tabs.visited, tabs.activeKey)
        expect(JSON.stringify(snapshot)).not.toMatch(/secret|token/)
        saveWorkspace(router, '7', tabs.visited, tabs.activeKey)
        expect(loadWorkspace(router, '8')).toBeNull()
        expect(loadWorkspace(router, '7')?.tabs[1]?.fullPath).toBe('/users?page_number=3')
    })
    it('损坏及无法写入的 storage 不妨碍导航', () => {
        localStorage.setItem('cobotusr:workspace:v1:7', 'bad')
        expect(loadWorkspace(router, '7')).toBeNull()
        vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
            throw Error('Quota')
        })
        expect(() => saveWorkspace(router, '7', [], '')).not.toThrow()
    })
    it('会话切换释放 owner 并清理确认队列', async () => {
        establishSession(7)
        const a = showModal({ title: 'a', content: 'a' }),
            b = showModal({ title: 'b', content: 'b' })
        resetSession()
        expect(sessionOwner.value).toBeNull()
        await expect(a).resolves.toBe(false)
        await expect(b).resolves.toBe(false)
    })
})
describe('异步生命周期', () => {
    it('初访只请求一次，乱序返回不会覆盖新页', async () => {
        const key = ref('2'),
            requests = [deferred<string>(), deferred<string>()],
            loader = vi
                .fn()
                .mockImplementationOnce(() => requests[0]!.promise)
                .mockImplementationOnce(() => requests[1]!.promise)
        const scope = effectScope()
        const query = scope.run(() => useAsyncQuery({ key, loader }))!
        expect(loader).toHaveBeenCalledTimes(1)
        key.value = '3'
        await nextTick()
        requests[1]!.resolve('page3')
        await queryFlush()
        requests[0]!.resolve('page2')
        await queryFlush()
        expect(query.data.value).toBe('page3')
        expect(query.status.value).toBe('ready')
        scope.stop()
    })
    it('失活中止读取，重新激活取消中的页只重试一次', async () => {
        const active = ref(true),
            loader = vi.fn(
                (signal: AbortSignal) =>
                    new Promise<string>((_, rej) =>
                        signal.addEventListener('abort', () => rej(Error('cancel'))),
                    ),
            )
        const scope = effectScope(),
            query = scope.run(() => useAsyncQuery({ key: 'a', active, loader }))!
        active.value = false
        await nextTick()
        expect(query.status.value).toBe('idle')
        active.value = true
        await nextTick()
        expect(loader).toHaveBeenCalledTimes(2)
        scope.stop()
    })
    it('相同查询刷新失败保留上次结果并可恢复', async () => {
        const loader = vi
            .fn()
            .mockResolvedValueOnce('cached')
            .mockRejectedValueOnce(Error('网络错误'))
            .mockResolvedValue('new')
        const scope = effectScope(),
            query = scope.run(() => useAsyncQuery<string>({ key: 'a', loader }))!
        await queryFlush()
        await query.refresh()
        expect(query.data.value).toBe('cached')
        expect(query.error.value).toBe('网络错误')
        await query.refresh()
        expect(query.data.value).toBe('new')
        scope.stop()
    })
    it('重复写入被拦截，旧会话写响应不调用成功回调', async () => {
        const wait = deferred<void>(),
            mutation = useAsyncMutation(),
            success = vi.fn()
        const first = mutation.run(() => wait.promise, success)
        await expect(mutation.run(() => Promise.resolve(), success)).resolves.toBe(false)
        resetSession()
        wait.resolve()
        await first
        expect(success).not.toHaveBeenCalled()
    })
    it('完整基线读取超过 1000 条，任一页失败拒绝伪完整数据', async () => {
        const signal = new AbortController().signal
        const loader = vi.fn(async (page: number, size: number) => ({
            page_number: page,
            page_size: size,
            total_pages: 12,
            data_list: [page],
        }))
        expect(await readAllPages(loader, signal)).toHaveLength(12)
        expect(loader).toHaveBeenCalledTimes(12)
        await expect(
            readAllPages(async () => {
                throw Error('failed')
            }, signal),
        ).rejects.toThrow('failed')
    })
})
describe('地址与确认', () => {
    it.each(['-1', '0', '1.2', 'abc', '999999999999999999999'])('无效页码 %s 归一化', (value) => {
        expect(normalizePageQuery({ page_number: value, page_size: '1000' })).toEqual({
            page_number: 1,
            page_size: 10,
        })
    })
    it('过滤数组、敏感值，保留合法筛选', () => {
        expect(normalizePageQuery({ q: '账户', roleId: '2', user_id: ['3'], secret: 'x' })).toEqual(
            { page_number: 1, page_size: 10, q: '账户', roleId: 2 },
        )
    })
    it('确认和取消都兑现 Promise，清理未结束队列', async () => {
        const a = showModal({ title: 'a', content: 'a' })
        settleConfirmation(true)
        await expect(a).resolves.toBe(true)
        const b = showModal({ title: 'b', content: 'b' })
        cleanupConfirmations()
        await expect(b).resolves.toBe(false)
    })
})
async function queryFlush() {
    await Promise.resolve()
    await Promise.resolve()
    await nextTick()
}
