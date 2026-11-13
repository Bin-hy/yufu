import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import Accounts from '@/pages/admin/views/accounts.vue'
import ConfirmHost from '@/components/Feedback/confirm-host.vue'
import { pinia } from '@/pinia'
import { useUserStore } from '@/pinia/stores/user'
import { useToastStore } from '@/pinia/stores/toast'
import { resetSession } from '@/workspace/session'
const mocked = vi.hoisted(() => ({ listAccounts: vi.fn(), setAccount: vi.fn() }))
vi.mock('@/pages/admin/api', () => ({ ...mocked, searchAccounts: vi.fn(), issueInvitation: vi.fn(), issueReset: vi.fn(), revokeAction: vi.fn(), setCandidate: vi.fn() }))
const account = { user_id: 21, username: 'administrator', nickname: '管理员甲', enabled: true, person_kind: 'employee', developer_enabled: true, revision: 3 }
let wrappers: VueWrapper[] = []
beforeEach(() => {
    resetSession(); vi.clearAllMocks()
    useUserStore(pinia).setCenterSession({ user_id: 12, sub: '12', username: 'operator', nickname: '操作人', person_kind: 'employee', developer_enabled: true, administrator: true, sso_session_id: 'root', auth_time: 10, expires_at: 9999999999, csrf_token: 'proof' })
    mocked.listAccounts.mockReset().mockResolvedValue({ items: [account], next: null })
    mocked.setAccount.mockReset()
})
afterEach(() => { wrappers.forEach(w => w.unmount()); wrappers = [] })
async function render() {
    const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/admin/accounts', name: 'admin-accounts', component: {} }] })
    await router.push('/admin/accounts')
    const page = mount(Accounts, { attachTo: document.body, global: { plugins: [pinia, router] } })
    const host = mount(ConfirmHost, { attachTo: document.body, global: { plugins: [pinia, router] } })
    wrappers.push(page, host); await flushPromises()
    return page
}
function dialogButton(label: string) {
    return [...document.querySelectorAll<HTMLButtonElement>('[role="alertdialog"] button')].find(b => b.textContent?.trim() === label)!
}
it('停用最后管理员失败留在原确认框，原因清晰且不产生全局 Toast', async () => {
    mocked.setAccount.mockRejectedValue({ response: { status: 409, data: { code: 'last_administrator', request_id: 'req-admin' } } })
    const page = await render()
    await page.findAll('.desktop-table button').find(b => b.text() === '停用')!.trigger('click'); await flushPromises()
    expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain('管理员甲')
    dialogButton('停用并撤销登录').click(); await flushPromises()
    expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain('最后一个有效管理员')
    expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain('req-admin')
    expect(mocked.setAccount).toHaveBeenCalledTimes(1)
    expect(useToastStore(pinia).toasts).toHaveLength(0)
    dialogButton('取消').click(); await flushPromises()
    expect(document.querySelector('[role="alertdialog"]')).toBeNull()
    expect(page.text()).toContain('最后一个有效管理员')
})
it('目录加载失败可重试，刷新故障保留原数据并标注，恢复后清除错误', async () => {
    mocked.listAccounts.mockRejectedValueOnce({ response: { status: 503, data: { code: 'dependency_unavailable' } } })
    const page = await render()
    expect(page.text()).toContain('加载失败')
    await page.findAll('button').find(b => b.text() === '重新加载')!.trigger('click'); await flushPromises()
    expect(page.text()).toContain('管理员甲')
    // Successful write followed by failed refresh must keep both the result and its explanation.
    mocked.setAccount.mockResolvedValue({})
    mocked.listAccounts.mockRejectedValueOnce({ response: { status: 503, data: { code: 'dependency_unavailable' } } })
    await page.findAll('.desktop-table button').find(b => b.text() === '停用')!.trigger('click'); await flushPromises()
    dialogButton('停用并撤销登录').click(); await flushPromises()
    expect(page.text()).toContain('刷新失败，以下为上次结果')
    expect(page.text()).toContain('管理员甲')
    expect(mocked.setAccount).toHaveBeenCalledTimes(1)
    await page.findAll('button').find(b => b.text() === '重试')!.trigger('click'); await flushPromises()
    expect(page.text()).not.toContain('刷新失败')
    expect(mocked.setAccount).toHaveBeenCalledTimes(1)
})
