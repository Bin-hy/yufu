import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { pinia } from '@/pinia'
import { useUserStore } from '@/pinia/stores/user'
import { resetSession } from '@/workspace/session'
import AppDetail from '@/pages/developer/views/app-detail.vue'
import type { EnvironmentConfig } from '@/pages/developer/api/type'
const mocked = vi.hoisted(() => ({
    getApp: vi.fn(), changeApp: vi.fn(), getReaders: vi.fn(), changeReader: vi.fn(), getApproval: vi.fn(), getAppDeliveries: vi.fn(),
    toast: vi.fn(), getCandidates: vi.fn(), getProviderMetadata: vi.fn(), getAssignments: vi.fn(), setAssignment: vi.fn(), setTestUser: vi.fn(),
}))
vi.mock('@/utils/toast', () => ({ emitToast: mocked.toast }))
vi.mock('@/pages/developer/api', () => ({
    getApp: mocked.getApp, changeApp: mocked.changeApp, getReaders: mocked.getReaders, changeReader: mocked.changeReader, getAppDeliveries: mocked.getAppDeliveries,
    getApproval: mocked.getApproval, getCandidates: mocked.getCandidates, getProviderMetadata: mocked.getProviderMetadata,
    listApps: vi.fn(), registerApp: vi.fn(), registerAdminApp: vi.fn(),
}))
vi.mock('@/pages/admin/api', () => ({ getAssignments: mocked.getAssignments, setAssignment: mocked.setAssignment, setTestUser: mocked.setTestUser }))
const identity = { user_id: 12, sub: '12', username: 'employee', nickname: 'Employee', person_kind: 'employee' as const, developer_enabled: true, administrator: true, sso_session_id: 'root-one', auth_time: 10, expires_at: 9999999999, csrf_token: 'proof' }
const config = (audience: string, redirect: string): EnvironmentConfig => ({ client_kind: 'server_web', redirect_uris: [redirect], post_logout_redirect_uris: [], browser_origins: [], access_audience: audience, identity_scopes: 'openid profile', consent_policy: 'user_consent', logout_capability: 'online', backchannel_logout_uri: null })
const active = config('urn:app:api', 'https://app.example/callback')
const application = {
    application_id: 1, app_key: 'app', name: 'App', description: null, management_mode: 'developer_managed' as const, third_party: false,
    active: true, revision: 3, owner_user_id: 12, members: [{ user_id: 12, member_role: 'owner' }],
    environments: [{ environment_id: 'prod', kind: 'production' as const, client_id: 'client', primary_client_kind: 'server_web' as const, secret_revision: 1, revision: 3, lifecycle_epoch: 1, active_version_id: 'v1', latest: { version_id: 'v2', version: 2, config: active, candidate_client_id: null }, active_config: active, pending_approval: 'ap1' }],
}
let wrappers: VueWrapper[] = []
beforeEach(() => {
    resetSession()
    for (const fn of Object.values(mocked)) fn.mockReset()
    useUserStore(pinia).setCenterSession(identity)
    mocked.getApp.mockResolvedValue(application)
    mocked.getReaders.mockResolvedValue({ items: [], next: null })
    mocked.getAppDeliveries.mockResolvedValue({ items: [], next: null })
    mocked.getAssignments.mockResolvedValue([])
    mocked.getProviderMetadata.mockResolvedValue({ issuer: 'https://id.example' })
})
afterEach(() => { for (const wrapper of wrappers) wrapper.unmount(); wrappers = [] })
async function render(path = '/admin/apps/1') {
    const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/admin/apps/:id', name: 'admin-app', component: {} }, { path: '/developer/apps/:id', name: 'developer-app', component: {} }] })
    await router.push(path)
    const wrapper = mount(AppDetail, { global: { plugins: [pinia, router] } })
    wrappers.push(wrapper)
    await flushPromises()
    return wrapper
}
it('审批页展示提交时固定的申请快照与当前生效配置的差异', async () => {
    mocked.getApproval.mockResolvedValue({
        approval_id: 'ap1', application_id: 1, environment_id: 'prod', version_id: 'v2', config: config('urn:app:api', 'https://app.example/new-callback'),
        snapshot: { owner_user_id: 12 }, snapshot_hash: 'hash', requested_by: 99, status: 'pending', reviewed_by: null, reason: null, logout_limitation_accepted: false,
    })
    const wrapper = await render()
    expect(mocked.getApproval).toHaveBeenCalledWith(true, 1, 'ap1', expect.any(AbortSignal))
    const text = wrapper.text()
    expect(text).toContain('固定申请快照对比')
    expect(text).toContain('提交时固定的版本 v2')
    expect(text).toContain('登录回调')
    expect(text).toContain('https://app.example/callback')
    expect(text).toContain('https://app.example/new-callback')
    expect(text).toContain('快照负责人账户 12')
    expect(text).toContain('退出限制已接受：否')
    expect(text).not.toContain('与当前生效配置一致。')
})
it('固定申请快照与生效配置一致时明确说明无差异', async () => {
    mocked.getApproval.mockResolvedValue({
        approval_id: 'ap1', application_id: 1, environment_id: 'prod', version_id: 'v2', config: active,
        snapshot: { owner_user_id: 12 }, snapshot_hash: 'hash', requested_by: 99, status: 'pending', reviewed_by: null, reason: null, logout_limitation_accepted: true,
    })
    const wrapper = await render()
    expect(wrapper.text()).toContain('与当前生效配置一致。')
    expect(wrapper.text()).toContain('退出限制已接受：是')
})
it('固定申请中的启动入口与说明必须列入审批差异', async () => {
    mocked.getApproval.mockResolvedValue({
        approval_id: 'ap1', application_id: 1, environment_id: 'prod', version_id: 'v2',
        config: { ...active, launch_url: 'https://app.example/workbench', launch_description: '业务工作台' },
        snapshot: { owner_user_id: 12 }, requested_by: 99, status: 'pending', logout_limitation_accepted: false,
    })
    const wrapper = await render()
    expect(wrapper.text()).toContain('应用启动地址')
    expect(wrapper.text()).toContain('https://app.example/workbench')
    expect(wrapper.text()).toContain('业务工作台')
})

it('生产申请拒绝提交未保存的编辑内容', async () => {
    useUserStore(pinia).setCenterSession({ ...identity, administrator: false })
    mocked.getApp.mockResolvedValue({ ...application, environments: [{ ...application.environments[0], pending_approval: null }] })
    const wrapper = await render('/developer/apps/1?environment=production')
    const submit = () => wrapper.findAll('button').find(button => button.text() === '提交已保存的最新版本供审核')!
    expect(submit().attributes('disabled')).toBeUndefined()
    await wrapper.get('#detail-config-launch-url').setValue('https://app.example/new-entry')
    expect(submit().attributes('disabled')).toBeDefined()
    expect(wrapper.text()).toContain('有未保存的修改')
    expect(mocked.changeApp).not.toHaveBeenCalled()
})
it('保存配置失败保留编辑内容且不显示成功提示', async () => {
    mocked.changeApp.mockRejectedValue(new Error('保存失败'))
    const wrapper = await render()
    await wrapper.get('#detail-config-launch-url').setValue('https://app.example/new-entry')
    const form = wrapper.findAll('form').find(form => form.find('#detail-config-launch-url').exists())!
    await form.trigger('submit')
    await flushPromises()
    expect(wrapper.text()).toContain('保存失败')
    expect((wrapper.get('#detail-config-launch-url').element as HTMLInputElement).value).toBe('https://app.example/new-entry')
    expect(mocked.toast).not.toHaveBeenCalledWith(expect.objectContaining({ type: 'success' }))
})
it('回执深链直接选中生产环境', async () => {
    mocked.getApp.mockResolvedValue({ ...application, environments: [{ ...application.environments[0], environment_id: 'dev', kind: 'development', pending_approval: null }, application.environments[0]] })
    const wrapper = await render('/admin/apps/1?environment=production')
    expect(wrapper.text()).toContain('环境 IDprod')
})
