import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { pinia } from '@/pinia'
import { useBrandingStore } from '@/pinia/stores/branding'
import { defaultDocument } from '@/branding/defaults'
import { resolveBranding } from '@/branding/resolve'
import { navigationFor } from '@/router/navigation'
import { canEnter } from '@/router/guard'
import Settings from '@/pages/admin/views/site-settings.vue'
import { resetSession } from '@/workspace/session'
import { resetDrafts, hasUnsavedWork } from '@/workspace/draft-registry'
const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn(), reset: vi.fn(), read: vi.fn(), upload: vi.fn() }))
const modal = vi.hoisted(() => vi.fn())
vi.mock('@/pages/admin/api/site-settings', () => ({ getSiteSettings: api.get, saveSiteSettings: api.save, resetSiteSettings: api.reset, readSiteAsset: api.read, uploadSiteAsset: api.upload }))
vi.mock('@/utils/modal', async () => ({ ...await vi.importActual<object>('@/utils/modal'), showModal: modal }))
const limits = { logo_bytes: 2097152, favicon_bytes: 2097152, cover_bytes: 10485760, site_name: 80, site_description: 300, welcome_title: 120, welcome_description: 1000, copyright_text: 300, filing_number: 100, links: 10, link_label: 80, link_url: 2048 }
const response = () => ({ settings: { document: defaultDocument(), revision: 1, updated_by: null, updated_at: null }, limits })
let wrapper: VueWrapper | undefined
beforeEach(() => { resetSession(); resetDrafts(); vi.clearAllMocks(); api.get.mockResolvedValue(response()); modal.mockResolvedValue(true); useBrandingStore(pinia).setPublished(resolveBranding(defaultDocument(), 1)) })
afterEach(() => { wrapper?.unmount(); wrapper = undefined; resetDrafts() })
async function render() {
    const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/admin/site-settings', name: 'admin-site-settings', component: Settings }] })
    await router.push('/admin/site-settings'); await router.isReady(); wrapper = mount(Settings, { global: { plugins: [pinia, router] } }); await flushPromises(); return wrapper
}
it('requires super administrator independently of enterprise administration', () => {
    for (const developer_enabled of [false, true]) for (const administrator of [false, true]) { const identity = { developer_enabled, administrator, super_administrator: false }; expect(canEnter({ qualification: 'super_administrator' }, identity)).toBe(false); expect(navigationFor(identity).some(g => g.items.some(i => i.name === 'admin-site-settings'))).toBe(false) }
    expect(canEnter({ qualification: 'super_administrator' }, { developer_enabled: false, administrator: false, super_administrator: true })).toBe(true)
})
it('keeps preview separate and sends the original revision on save', async () => {
    const w = await render(); const original = useBrandingStore(pinia).current.branding.site_name; await w.get('#site-name').setValue('新企业名称')
    expect(hasUnsavedWork.value).toBe(true); expect(useBrandingStore(pinia).current.branding.site_name).toBe(original); expect(w.find('.site-preview').text()).toContain('新企业名称')
    const result = response(); result.settings.revision = 2; result.settings.document.branding.site_name = '新企业名称'; api.save.mockResolvedValue(result)
    await w.get('form').trigger('submit'); await flushPromises()
    expect(api.save).toHaveBeenCalledWith(expect.objectContaining({ branding: expect.objectContaining({ site_name: '新企业名称' }) }), 1); expect(useBrandingStore(pinia).current.branding.site_name).toBe('新企业名称'); expect(hasUnsavedWork.value).toBe(false)
})
it('retains draft on conflict without reloading or overwriting', async () => {
    const w = await render(); await w.get('#site-name').setValue('冲突草稿'); api.save.mockRejectedValue({ response: { status: 409 } })
    await w.get('form').trigger('submit'); await flushPromises(); expect(w.text()).toContain('草稿已保留'); expect((w.get('#site-name').element as HTMLInputElement).value).toBe('冲突草稿'); expect(api.get).toHaveBeenCalledTimes(1)
})
it('keeps the draft and published branding visible when validation fails', async () => {
    const w = await render(); await w.get('#site-name').setValue('保留的输入'); api.save.mockRejectedValue(new Error('网站名称不能为空'))
    await w.get('form').trigger('submit'); await flushPromises()
    expect(w.text()).toContain('网站名称不能为空'); expect((w.get('#site-name').element as HTMLInputElement).value).toBe('保留的输入')
    expect(useBrandingStore(pinia).current.branding.site_name).toBe('yufu'); expect(hasUnsavedWork.value).toBe(true)
})
it('does not show editable defaults when management reads fail', async () => {
    api.get.mockRejectedValue(new Error('无权读取')); const w = await render(); expect(w.text()).toContain('无权读取'); expect(w.find('form').exists()).toBe(false)
})
it('requires confirmation and never requests a cancelled reset', async () => {
    const w = await render(); modal.mockResolvedValue(false)
    await w.findAll('button').find(b => b.text() === '恢复默认')!.trigger('click'); await flushPromises(); expect(api.reset).not.toHaveBeenCalled()
    api.reset.mockResolvedValue({ ...response(), settings: { ...response().settings, revision: 2 } }); modal.mockResolvedValue(true)
    await w.findAll('button').find(b => b.text() === '恢复默认')!.trigger('click'); await flushPromises(); expect(api.reset).toHaveBeenCalledWith(1); expect(w.text()).toContain('当前版本 2')
})
