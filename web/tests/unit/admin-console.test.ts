import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory, type Router } from 'vue-router'
import type { Component } from 'vue'
import Accounts from '@/pages/admin/views/accounts.vue'
import Groups from '@/pages/admin/views/groups.vue'
import Audit from '@/pages/admin/views/audit.vue'
import Deliveries from '@/pages/admin/views/deliveries.vue'
import Overview from '@/pages/admin/views/overview.vue'
import EntityPicker from '@/components/ui/entity-picker/entity-picker.vue'
import ConfirmHost from '@/components/Feedback/confirm-host.vue'
import { pinia } from '@/pinia'
import { useUserStore } from '@/pinia/stores/user'
import { resetSession } from '@/workspace/session'

const mocked = vi.hoisted(() => ({
    listAccounts: vi.fn(), searchAccounts: vi.fn(), setAccount: vi.fn(), issueInvitation: vi.fn(),
    issueReset: vi.fn(), revokeAction: vi.fn(), setCandidate: vi.fn(), listGroups: vi.fn(),
    groupChange: vi.fn(), searchGroups: vi.fn(), listAudit: vi.fn(), searchAudit: vi.fn(),
    listDeliveries: vi.fn(), retryDelivery: vi.fn(), getOverview: vi.fn(), listMigrations: vi.fn(),
    scanMigrations: vi.fn(), adoptLegacy: vi.fn(), getAssignments: vi.fn(), setAssignment: vi.fn(), setTestUser: vi.fn(),
}))
vi.mock('@/pages/admin/api', () => mocked)
const overview = {
    accounts: { total: 8, disabled: 2, developers: 3 },
    deliveries: { pending: 1, leased: 0, retry: 2, delivered: 5, permanent_failure: 3, expired: 1 },
    approvals: { pending: 4 },
    migrations: { unmanaged: 2, unresolved_conflicts: 1 },
    audit: { recent_failures: 6 },
}
let wrappers: VueWrapper[] = []
beforeEach(() => {
    resetSession(); vi.clearAllMocks()
    useUserStore(pinia).setCenterSession({ user_id: 12, sub: '12', username: 'operator', nickname: '操作人', person_kind: 'employee', developer_enabled: true, administrator: true, sso_session_id: 'root', auth_time: 10, expires_at: 9999999999, csrf_token: 'proof' })
    mocked.listAccounts.mockResolvedValue({ items: [], next: null })
    mocked.listGroups.mockResolvedValue({ items: [], next: null })
    mocked.searchAudit.mockResolvedValue({ items: [], next: null })
    mocked.listDeliveries.mockResolvedValue({ items: [], next: null })
    mocked.getOverview.mockResolvedValue(overview)
})
afterEach(() => { wrappers.forEach(w => w.unmount()); wrappers = [] })
async function render(component: Component, path: string): Promise<{ page: VueWrapper; router: Router }> {
    const router = createRouter({ history: createMemoryHistory(), routes: [
        { path: '/admin', name: 'admin-overview', component: {} },
        { path: '/admin/accounts', name: 'admin-accounts', component: {} },
        { path: '/admin/groups', name: 'admin-groups', component: {} },
        { path: '/admin/apps', name: 'admin-apps', component: {} },
        { path: '/admin/apps/:id', name: 'admin-app', component: {} },
        { path: '/admin/audit', name: 'admin-audit', component: {} },
        { path: '/admin/deliveries', name: 'admin-deliveries', component: {} },
        { path: '/admin/migrations', name: 'admin-migrations', component: {} },
        { path: '/account/action', name: 'account-action', component: {} },
    ] })
    await router.push(path)
    const page = mount(component, { attachTo: document.body, global: { plugins: [pinia, router] } })
    const host = mount(ConfirmHost, { attachTo: document.body, global: { plugins: [pinia, router] } })
    wrappers.push(page, host); await flushPromises()
    return { page, router }
}
function dialogButton(label: string) {
    return [...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')].find(b => b.textContent?.trim() === label)!
}

it('账户状态筛选在服务端执行并写入地址栏，批量停用经确认后逐个提交', async () => {
    const accounts = [
        { user_id: 21, username: 'a', nickname: '甲', enabled: true, person_kind: 'employee', developer_enabled: false, revision: 3 },
        { user_id: 22, username: 'b', nickname: '乙', enabled: true, person_kind: 'employee', developer_enabled: false, revision: 7 },
    ]
    mocked.listAccounts.mockResolvedValue({ items: accounts, next: null })
    const { page, router } = await render(Accounts, '/admin/accounts')
    await page.findAll('.filter-chip').find(b => b.text() === '停用')!.trigger('click')
    await flushPromises()
    expect(router.currentRoute.value.query.status).toBe('disabled')
    const lastCall = mocked.listAccounts.mock.calls.at(-1)!
    expect(lastCall[3]).toBe(false)
    await page.findAll('.filter-chip').find(b => b.text() === '全部')!.trigger('click')
    await flushPromises()
    expect(mocked.listAccounts.mock.calls.at(-1)![3]).toBeUndefined()

    mocked.setAccount.mockResolvedValue({})
    await page.find('thead input[type="checkbox"]').setValue(true)
    await page.findAll('button').find(b => b.text() === '批量停用')!.trigger('click')
    await flushPromises()
    expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain('2 个账户')
    dialogButton('停用 2 个账户').click(); await flushPromises()
    expect(mocked.setAccount).toHaveBeenCalledTimes(2)
    expect(mocked.setAccount).toHaveBeenCalledWith(21, 'enabled', false, 3)
    expect(mocked.setAccount).toHaveBeenCalledWith(22, 'enabled', false, 7)
})
it('移除组成员需要确认并说明影响，批量添加成员沿用回执中的新修订号', async () => {
    const group = {
        group_id: 'f1e2d3c4-0000-0000-0000-000000000001', name: '数据平台', description: null, revision: 3,
        members: [31], member_profiles: [{ user_id: 31, username: 'member', nickname: '成员甲' }],
        application_ids: [], application_labels: [],
    }
    mocked.listGroups.mockResolvedValue({ items: [group], next: null })
    mocked.groupChange.mockResolvedValueOnce({ operation_id: 'op1', summary: { revision: 4 } })
        .mockResolvedValueOnce({ operation_id: 'op2', summary: { revision: 5 } })
    const { page } = await render(Groups, '/admin/groups')
    await page.find('button[aria-label="移除成员 成员甲"]').trigger('click')
    await flushPromises()
    expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain('应用访问会一并失效')
    dialogButton('取消').click(); await flushPromises()
    expect(mocked.groupChange).not.toHaveBeenCalled()

    const picker = page.findComponent(EntityPicker)
    picker.vm.$emit('select', { value: 41, label: '新成员一', secondary: '' })
    picker.vm.$emit('select', { value: 42, label: '新成员二', secondary: '' })
    await flushPromises()
    await page.findAll('button').find(b => b.text().includes('添加 2 名成员'))!.trigger('click')
    await flushPromises()
    expect(mocked.groupChange).toHaveBeenCalledTimes(2)
    expect(mocked.groupChange.mock.calls[0]![2]).toEqual({ present: true, expected_revision: 3 })
    expect(mocked.groupChange.mock.calls[1]![2]).toEqual({ present: true, expected_revision: 4 })
})
it('审计筛选传给服务端搜索接口，时间倒序字段齐全', async () => {
    mocked.searchAudit.mockResolvedValue({ items: [
        { id: 'e1', request_id: 'req-1', actor_id: 12, action: 'account.disable', object_kind: 'account', object_id: '21', happened_at: Math.floor(Date.now() / 1000) - 120, result: 'success', reason_code: null, change_summary: {} },
    ], next: null })
    const { page, router } = await render(Audit, '/admin/audit')
    expect(page.text()).toContain('account.disable')
    expect(page.text()).toContain('分钟前')
    await page.findAll('.filter-chip').find(b => b.text() === '失败')!.trigger('click')
    await flushPromises()
    expect(router.currentRoute.value.query.result).toBe('failure')
    const filter = mocked.searchAudit.mock.calls.at(-1)![0]
    expect(filter.result).toBe('failure')
    expect(filter.action).toBeUndefined()
})
it('退出通知状态筛选走服务端，统计卡展示全库数字，重试只锁定当前行', async () => {
    mocked.listDeliveries.mockResolvedValue({ items: [
        { id: 'd1', event_id: 'e1', application_id: 1, name: '订单系统', environment: 'production', status: 'permanent_failure', attempt_count: 3, next_attempt_at: 0, last_error_code: 'timeout', created_at: 100, delivered_at: null },
        { id: 'd2', event_id: 'e2', application_id: 1, name: '订单系统', environment: 'production', status: 'delivered', attempt_count: 1, next_attempt_at: 0, last_error_code: null, created_at: 100, delivered_at: 200 },
    ], next: null })
    const { page, router } = await render(Deliveries, '/admin/deliveries')
    expect(page.text()).toContain('需要关注')
    expect(page.text()).toContain('失败 3 · 待重试 2 · 过期 1')
    const retryButtons = page.findAll('.desktop-table button').filter(b => b.text() === '按原目标重新排队')
    expect(retryButtons).toHaveLength(1)
    await page.findAll('.filter-chip').find(b => b.text() === '发送失败')!.trigger('click')
    await flushPromises()
    expect(router.currentRoute.value.query.status).toBe('permanent_failure')
    expect(mocked.listDeliveries.mock.calls.at(-1)![2]).toBe('permanent_failure')
})
it('管理总览聚合待办并按权限隐藏区块', async () => {
    mocked.getOverview.mockResolvedValue({ ...overview, audit: null })
    const { page } = await render(Overview, '/admin')
    expect(page.text()).toContain('待审批的生产配置')
    expect(page.text()).toContain('待纳管的历史客户端')
    expect(page.text()).not.toContain('近 24 小时审计')
    expect(page.text()).toContain('停用 2 · 开发者 3')
})
