import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { nextTick, type Component } from 'vue'
import { pinia } from '@/pinia'
import { useUserStore } from '@/pinia/stores/user'
import { resetSession } from '@/workspace/session'
import { consumeSecretFragment } from '@/pages/account/api/actions'
import { navigationFor } from '@/router/navigation'
import Profile from '@/pages/account/views/profile.vue'
import Sessions from '@/pages/account/views/sessions.vue'
import Apps from '@/pages/developer/views/apps.vue'
import Consent from '@/pages/oidc-consent/index.vue'
import AccountAction from '@/pages/account/views/action.vue'
const mocked = vi.hoisted(() => ({
    session: vi.fn(), profile: vi.fn(), password: vi.fn(), centers: vi.fn(), rps: vi.fn(), revokeCenter: vi.fn(), revokeRp: vi.fn(), logout: vi.fn(),
    apps: vi.fn(), register: vi.fn(), registerAdmin: vi.fn(), inspect: vi.fn(), complete: vi.fn(), exchange: vi.fn(), action: vi.fn(), accept: vi.fn(), reset: vi.fn(),
}))
vi.mock('@/pages/account/api', () => ({ getCenterSession: mocked.session, updateProfile: mocked.profile, changePassword: mocked.password, getCenters: mocked.centers, getRps: mocked.rps, revokeCenter: mocked.revokeCenter, revokeRp: mocked.revokeRp, centerLogout: mocked.logout }))
vi.mock('@/pages/developer/api', () => ({ listApps: mocked.apps, registerApp: mocked.register, registerAdminApp: mocked.registerAdmin }))
vi.mock('@/pages/oidc-consent/api', () => ({ inspectFlow: mocked.inspect, completeFlow: mocked.complete }))
vi.mock('@/pages/account/api/actions', async () => ({ ...await vi.importActual<object>('@/pages/account/api/actions'), exchangeAction: mocked.exchange, inspectAction: mocked.action, acceptInvitation: mocked.accept, completePasswordReset: mocked.reset }))
const identity = { user_id: 12, sub: '12', username: 'employee', nickname: 'Employee', person_kind: 'employee' as const, developer_enabled: true, administrator: false, sso_session_id: 'root-one', auth_time: 10, expires_at: 9999999999, csrf_token: 'proof' }
let wrappers: VueWrapper[] = []
beforeEach(() => { resetSession(); for (const fn of Object.values(mocked)) fn.mockReset(); mocked.session.mockResolvedValue(identity); mocked.apps.mockResolvedValue({ items: [], next: null }); useUserStore(pinia).setCenterSession(identity) })
afterEach(() => { for (const wrapper of wrappers) wrapper.unmount(); wrappers = []; window.history.replaceState({}, '', '/') })
async function render(component: Component, path = '/account') {
    const router = createRouter({ history: createMemoryHistory(), routes: [
        { path: '/account', name: 'account-profile', component: {} }, { path: '/login', name: 'login', component: {} },
        { path: '/account/action', name: 'account-action', component: {} }, { path: '/register', name: 'register', component: {} },
        { path: '/account/sessions', name: 'account-sessions', component: {} },
        { path: '/developer/apps', name: 'developer-apps', component: {} }, { path: '/developer/apps/:id', name: 'developer-app', component: {} },
        { path: '/admin/apps', name: 'admin-apps', component: {} }, { path: '/admin/apps/:id', name: 'admin-app', component: {} }, { path: '/oidc/consent', name: 'oidc-consent', component: {} },
    ] })
    await router.push(path)
    const wrapper = mount(component, { attachTo: document.body, global: { plugins: [pinia, router] } }); wrappers.push(wrapper)
    await flushPromises(); return { wrapper, router }
}
async function openRegistration(wrapper: VueWrapper, action: string) {
    await wrapper.findAll('button').find(b => b.text() === action)!.trigger('click')
    await nextTick()
}
async function nextRegistrationStep(wrapper: VueWrapper) {
    await wrapper.findAll('button').find(b => b.text() === '下一步')!.trigger('click')
    await flushPromises()
}
async function fillBasicStep(wrapper: VueWrapper, key = 'my-app', name = 'My App') {
    await wrapper.find('#app-key').setValue(key)
    await wrapper.find('#app-name').setValue(name)
}
async function fillEnvironmentStep(wrapper: VueWrapper, prefix: string, redirects: string, audience = 'urn:my-app:api') {
    await wrapper.find(`#${prefix}-redirects`).setValue(redirects)
    await wrapper.find(`#${prefix}-audience`).setValue(audience)
}
it('资料保存失败显示错误，改密成功才清会话并导航登录', async () => {
    mocked.profile.mockRejectedValue(new Error('profile rejected'))
    const { wrapper, router } = await render(Profile)
    await wrapper.find('#account-nickname').setValue('New Name')
    await wrapper.find('form').trigger('submit'); await flushPromises()
    expect(wrapper.text()).toContain('profile rejected')
    expect(useUserStore(pinia).centerSession?.nickname).toBe('Employee')
    await wrapper.find('#old-password').setValue('old-password')
    await wrapper.find('#new-password').setValue('new-password')
    await wrapper.find('#confirm-password').setValue('new-password')
    mocked.password.mockRejectedValueOnce(new Error('wrong old password'))
    await wrapper.findAll('form')[1]!.trigger('submit'); await flushPromises()
    expect(wrapper.text()).toContain('wrong old password'); expect(useUserStore(pinia).centerSession).not.toBeNull()
    expect((wrapper.find('#old-password').element as HTMLInputElement).value).toBe('')
    mocked.password.mockResolvedValue({ requires_login: true, user_id: 12, oidc_flow_id: null })
    for (const [selector, value] of [['#old-password', 'old-password'], ['#new-password', 'new-password'], ['#confirm-password', 'new-password']]) await wrapper.find(selector!).setValue(value!)
    await wrapper.findAll('form')[1]!.trigger('submit'); await flushPromises()
    expect(useUserStore(pinia).centerSession).toBeNull(); expect(router.currentRoute.value.path).toBe('/login')
})
it('会话列表区分中心和应用，撤销失败保留原结果且不伪报退出', async () => {
    mocked.centers.mockResolvedValue({ items: [{ id: 'root-one', current: true, auth_time: 10, expires_at: 9999999999, revoked_at: null }], next: null })
    mocked.rps.mockResolvedValue({ items: [{ id: 'rp-one', center_id: 'root-one', name: 'Business A', environment: 'production', created_at: 10, revoked_at: null, center_expires_at: 9999999999, center_revoked: false }], next: null })
    mocked.revokeRp.mockRejectedValue(new Error('revoke unavailable'))
    const { wrapper, router } = await render(Sessions, '/account/sessions')
    const revoke = wrapper.findAll('button').find(button => button.text() === '撤销此应用会话')!
    await revoke.trigger('click'); await flushPromises()
    expect(mocked.revokeRp).toHaveBeenCalledWith('rp-one')
    expect(mocked.revokeCenter).not.toHaveBeenCalled(); expect(mocked.logout).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('revoke unavailable'); expect(wrapper.text()).toContain('Business A')
    expect(router.currentRoute.value.path).toBe('/account/sessions')
    expect(useUserStore(pinia).centerSession).not.toBeNull()
})
it('注册失败重试使用同一操作键，秘密仅在一次结果中显示并随会话清理', async () => {
    mocked.register.mockRejectedValueOnce(new Error('commit uncertain')).mockResolvedValue({ operation_id: 'op-one', application_id: 1, environments: [{ environment_id: 'dev', kind: 'development' }], secrets: [{ environment_id: 'dev', client_secret: 'once-only-fixture-secret' }] })
    const { wrapper } = await render(Apps, '/developer/apps')
    await openRegistration(wrapper, '注册应用')
    await fillBasicStep(wrapper)
    await nextRegistrationStep(wrapper)
    await fillEnvironmentStep(wrapper, 'registration-dev', 'https://app.example/callback')
    await nextRegistrationStep(wrapper)
    await fillEnvironmentStep(wrapper, 'registration-prod', 'https://app.example/callback')
    await nextRegistrationStep(wrapper)
    await wrapper.find('form').trigger('submit'); await flushPromises()
    expect(wrapper.text()).toContain('commit uncertain'); expect(wrapper.text()).not.toContain('once-only-fixture-secret')
    await wrapper.find('form').trigger('submit'); await flushPromises()
    expect(mocked.register.mock.calls[0]![1]).toBe(mocked.register.mock.calls[1]![1])
    expect(mocked.register.mock.calls[1]![0].production.consent_policy).toBe('user_consent')
    expect(wrapper.text()).toContain('once-only-fixture-secret')
    expect(localStorage.length).toBe(0); expect(sessionStorage.length).toBe(0)
    resetSession(); await nextTick(); expect(wrapper.text()).not.toContain('once-only-fixture-secret')
})
it('开发者注册按环境区分回调必填与 https 规则，生产草稿可留空提交', async () => {
    mocked.register.mockResolvedValue({ operation_id: 'op-scheme', application_id: 3, environments: [] })
    const { wrapper } = await render(Apps, '/developer/apps')
    await openRegistration(wrapper, '注册应用')
    await fillBasicStep(wrapper, 'yufu-spa-example', 'yufu-spa-example')
    await nextRegistrationStep(wrapper)
    const dev = wrapper.find('#registration-dev-redirects')
    let prod = wrapper.find('#registration-prod-redirects')
    expect((dev.element as HTMLTextAreaElement).required).toBe(true)
    expect(wrapper.text()).toContain('本机调试可使用 http://localhost')
    await dev.setValue('http://127.0.0.1:5173/callback')
    await wrapper.find('#registration-dev-audience').setValue('urn:app:api')
    await nextRegistrationStep(wrapper)
    prod = wrapper.find('#registration-prod-redirects')
    expect((prod.element as HTMLTextAreaElement).required).toBe(false)
    expect(wrapper.text()).toContain('可先留空，生产配置经审批后生效；填写时必须使用 https')
    await wrapper.find('#registration-prod-audience').setValue('urn:app:api')
    await nextRegistrationStep(wrapper)
    await wrapper.find('form').trigger('submit'); await flushPromises()
    expect(mocked.register).toHaveBeenCalledTimes(1)
    expect(mocked.register.mock.calls[0]![0].development.redirect_uris).toEqual(['http://127.0.0.1:5173/callback'])
    expect(mocked.register.mock.calls[0]![0].production.redirect_uris).toEqual([])
})
it('管理员注册的生产回调必填且不显示草稿留空提示', async () => {
    const { wrapper } = await render(Apps, '/admin/apps')
    await openRegistration(wrapper, '配置获批第三方')
    await fillBasicStep(wrapper)
    await nextRegistrationStep(wrapper)
    await fillEnvironmentStep(wrapper, 'registration-dev', 'https://app.example/callback')
    await nextRegistrationStep(wrapper)
    expect((wrapper.find('#registration-prod-redirects').element as HTMLTextAreaElement).required).toBe(true)
    expect((wrapper.find('#registration-prod-redirects').element as HTMLTextAreaElement).required).toBe(true)
    expect(wrapper.text()).toContain('必须使用 https；http 地址（包括 127.0.0.1）会被拒绝')
    expect(wrapper.text()).not.toContain('可先留空')
})
it('进入确认前本地校验拦截生产 http 回调，行内显示错误且不发起请求', async () => {
    const { wrapper } = await render(Apps, '/developer/apps')
    await openRegistration(wrapper, '注册应用')
    await fillBasicStep(wrapper, 'yufu-spa-example', 'yufu-spa-example')
    await nextRegistrationStep(wrapper)
    await fillEnvironmentStep(wrapper, 'registration-dev', 'http://127.0.0.1:5173/callback', 'urn:app:api')
    await nextRegistrationStep(wrapper)
    await fillEnvironmentStep(wrapper, 'registration-prod', 'http://127.0.0.1:5173/callback', 'urn:app:api')
    await nextRegistrationStep(wrapper)
    expect(wrapper.find('#registration-step-title').text()).toBe('生产环境')
    expect(wrapper.find('#registration-prod-redirect_uris-error').text()).toContain('生产环境「登录回调地址」 http://127.0.0.1:5173/callback')
    expect(wrapper.text()).toContain('生产环境必须使用 https')
    expect(mocked.register).not.toHaveBeenCalled()
})
it('注册向导在基础信息缺失时停留在当前步骤并聚焦第一个错误字段', async () => {
    const { wrapper } = await render(Apps, '/developer/apps')
    await openRegistration(wrapper, '注册应用')
    await nextRegistrationStep(wrapper)
    expect(wrapper.text()).toContain('基础信息')
    expect(wrapper.find('#registration-dev-redirects').exists()).toBe(false)
    expect(wrapper.find('#app-key-error').text()).toContain('「应用 Key」不能为空')
    expect(document.activeElement?.id).toBe('app-key')
})
it('账户邀请秘密在网络请求前从普通与 hash 路由地址删除，不接受自选回调', () => {
    for (const hash of ['#kind=invitation&secret=' + 'A'.repeat(43), '#/account/action#kind=password_reset&secret=' + 'A'.repeat(43)]) {
        const history = { state: {}, replaceState: vi.fn() }
        const result = consumeSecretFragment({ hash, pathname: '/', search: '?flow_id=flow' }, history)
        expect(result?.secret).toHaveLength(43)
        expect(history.replaceState.mock.calls[0]![2]).not.toContain('secret')
        expect(history.replaceState.mock.calls[0]![2]).toBe(hash.startsWith('#/') ? '/?flow_id=flow#/account/action' : '/?flow_id=flow')
    }
    const history = { state: {}, replaceState: vi.fn() }
    expect(() => consumeSecretFragment({ hash: '#secret=bad&kind=evil', pathname: '/', search: '' }, history)).toThrow('无效')
    expect(history.replaceState).toHaveBeenCalled()
})
it('没有企业邀请的注册页面不能调用创建账户接口', async () => {
    useUserStore(pinia).logout()
    const { wrapper } = await render(AccountAction, '/register')
    expect(wrapper.text()).toContain('需要有效的企业邀请')
    expect(mocked.accept).not.toHaveBeenCalled(); expect(mocked.exchange).not.toHaveBeenCalled()
    expect(wrapper.find('form').exists()).toBe(false)
})
it('授权界面仅使用服务端 flow 摘要，拒绝时不透传 query 中的回调和权限', async () => {
    mocked.inspect.mockResolvedValue({ flow_id: 'flow-one', application_name: 'Trusted A', scopes: 'openid profile', interaction: 'consent', expires_at: 9999999999, username: 'employee', csrf_token: 'proof' })
    mocked.complete.mockRejectedValue(new Error('flow expired'))
    const { wrapper } = await render(Consent, '/oidc/consent?flow_id=flow-one&client_id=evil&redirect_uri=https://evil.test&scope=admin')
    expect(mocked.inspect.mock.calls[0]![0]).toBe('flow-one'); expect(wrapper.text()).toContain('Trusted A'); expect(wrapper.text()).not.toContain('evil')
    await wrapper.findAll('button').find(b => b.text() === '拒绝授权')!.trigger('click'); await flushPromises()
    expect(mocked.complete).toHaveBeenCalledWith('flow-one', false); expect(wrapper.text()).toContain('flow expired')
})
it('三个入口按当前资格显示，合作方无企业目录入口', () => {
    expect(navigationFor({ developer_enabled: false, administrator: false }).map(group => group.title)).toEqual(['账户中心'])
    expect(navigationFor({ developer_enabled: true, administrator: false }).map(group => group.title)).toEqual(['账户中心', '开发者控制台'])
    expect(navigationFor({ developer_enabled: false, administrator: true }).map(group => group.title)).toEqual(['账户中心', '企业管理'])
})
