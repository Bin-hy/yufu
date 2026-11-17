import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defaultDocument } from '@/branding/defaults'
import { resolveBranding, validatePublicBranding, safeLink } from '@/branding/resolve'
import { useBrandingStore } from '@/pinia/stores/branding'
import SiteLogo from '@/components/brand/site-logo.vue'
import SiteFooter from '@/components/brand/site-footer.vue'
import AuthVisual from '@/components/brand/auth-visual.vue'
beforeEach(() => setActivePinia(createPinia()))
afterEach(() => vi.unstubAllGlobals())
it('resolves independent overrides and explicit empty text without mutating defaults', () => {
    const d = defaultDocument(); d.branding.cover_asset_id = '11111111-1111-4111-8111-111111111111'; d.oidc.welcome_title = ''; d.oidc.welcome_description = 'OIDC'
    const resolved = resolveBranding(d, 2)
    expect(resolved.oidc.cover_url).toBe(resolved.login.cover_url); expect(resolved.oidc.welcome_title).toBe(''); expect(resolved.oidc.welcome_description).toBe('OIDC')
    d.oidc.welcome_title = null; expect(resolveBranding(d).oidc.welcome_title).toBe(d.branding.welcome_title)
    expect(defaultDocument().branding.cover_asset_id).toBeNull()
})
it('validates the public contract and rejects unsafe URLs', () => {
    const value = resolveBranding(defaultDocument(), 1); expect(validatePublicBranding(value)).toBe(true)
    expect(validatePublicBranding({ ...value, revision: NaN })).toBe(false)
    value.branding.logo_url = 'https://elsewhere.example/logo.png'; expect(validatePublicBranding(value)).toBe(false)
    for (const link of ['javascript:alert(1)', 'data:text/html,test', '/relative', 'https://user:pass@example.com']) expect(safeLink(link)).toBe(false)
    expect(safeLink('https://example.com')).toBe(true)
})
it('renders footer as text and suppresses empty or unsafe links', () => {
    const config = defaultDocument().branding.footer
    config.copyright_text = '<script>danger()</script>'; config.filing_number = '备案 123'; config.filing_url = 'javascript:alert(1)'
    config.links = [{ label: '帮助', url: 'https://example.com' }, { label: '危险', url: 'data:text/html,test' }]
    const w = mount(SiteFooter, { props: { config } })
    expect(w.find('script').exists()).toBe(false); expect(w.text()).toContain('<script>'); expect(w.findAll('a')).toHaveLength(1); expect(w.find('a').attributes('rel')).toContain('noopener'); w.unmount()
    const empty = mount(SiteFooter, { props: { config: { copyright_text: '', filing_number: '', filing_url: null, links: [] } } }); expect(empty.find('footer').exists()).toBe(false); empty.unmount()
})
it('falls back from failed logo and cover and retries a new URL', async () => {
    const w = mount(SiteLogo, { props: { name: '企业', url: '/bad.png' } }); await w.find('img').trigger('error')
    expect(w.find('img').exists()).toBe(false); expect(w.find('svg').exists()).toBe(true)
    await w.setProps({ url: '/next.png' }); expect(w.find('img').attributes('src')).toBe('/next.png'); w.unmount()
    const data = resolveBranding(defaultDocument()); const visual = mount(AuthVisual, { props: { branding: data.branding, appearance: { ...data.login, cover_url: '/bad.png' } } })
    await visual.find('.site-auth-cover').trigger('error'); expect(visual.find('.site-auth-cover').exists()).toBe(false); visual.unmount()
})
it('updates title after loading and ignores reads overtaken by a save', async () => {
    let complete: (v: unknown) => void
    vi.stubGlobal('fetch', vi.fn(() => new Promise(resolve => { complete = resolve })))
    const store = useBrandingStore(); store.setPageTitle('账户中心'); const pending = store.refresh()
    const saved = resolveBranding(defaultDocument(), 3); saved.branding.site_name = '企业平台'; store.setPublished(saved)
    complete!({ ok: true, json: async () => resolveBranding(defaultDocument(), 1) }); await pending
    expect(document.title).toBe('账户中心 · 企业平台'); expect(store.current.revision).toBe(3)
    store.setPageTitle('站点设置'); expect(document.title).toBe('站点设置 · 企业平台')
})
it('publishes the configured favicon and falls back when it fails to load', () => {
    const store = useBrandingStore(), value = resolveBranding(defaultDocument(), 2)
    value.branding.favicon_url = '/api/v1/site/assets/11111111-1111-4111-8111-111111111111'
    store.setPublished(value)
    const link = document.querySelector<HTMLLinkElement>('link[rel="icon"]')!
    expect(link.getAttribute('href')).toBe(value.branding.favicon_url)
    link.dispatchEvent(new Event('error'))
    expect(link.getAttribute('href')).toBe('/favicon.svg')
})
it('uses default appearance on a public service failure', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('unavailable')))
    const store = useBrandingStore(); await store.refresh(); expect(store.current.branding.site_name).toBe('yufu')
    expect(document.querySelector('link[rel="icon"]')?.getAttribute('href')).toBe('/favicon.svg')
})
