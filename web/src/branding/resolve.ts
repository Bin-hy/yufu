import type { PublicSiteBranding, SiteSettingsDocument, FooterConfig } from './types'
export function resolveBranding(document: SiteSettingsDocument, revision = 0, asset = (id: string) => `/api/v1/site/assets/${encodeURIComponent(id)}`): PublicSiteBranding {
    const b = document.branding
    const url = (id: string | null) => id ? asset(id) : null
    return {
        revision,
        branding: { site_name: b.site_name, site_description: b.site_description, logo_url: url(b.logo_asset_id), favicon_url: url(b.favicon_asset_id), footer: JSON.parse(JSON.stringify(b.footer)) },
        login: { cover_url: url(b.cover_asset_id), welcome_title: b.welcome_title, welcome_description: b.welcome_description },
        oidc: { cover_url: url(document.oidc.cover_asset_id ?? b.cover_asset_id), welcome_title: document.oidc.welcome_title ?? b.welcome_title, welcome_description: document.oidc.welcome_description ?? b.welcome_description },
    }
}
export function safeLink(value: unknown): value is string {
    if (typeof value !== 'string' || value.length > 2048 || value.trim() !== value) return false
    try { const u = new URL(value); return ['http:', 'https:'].includes(u.protocol) && !!u.hostname && !u.username && !u.password } catch { return false }
}
function footer(value: unknown): value is FooterConfig {
    const f = value as FooterConfig
    return !!f && typeof f.copyright_text === 'string' && typeof f.filing_number === 'string' && (f.filing_url === null || safeLink(f.filing_url)) && Array.isArray(f.links) && f.links.length <= 10 && f.links.every(l => typeof l?.label === 'string' && safeLink(l.url))
}
function image(value: unknown) { return value === null || (typeof value === 'string' && /^\/api\/v1\/site\/assets\/[a-f0-9-]{36}$/.test(value)) }
export function validatePublicBranding(value: unknown): value is PublicSiteBranding {
    const v = value as PublicSiteBranding
    return !!v && Number.isSafeInteger(v.revision) && v.revision >= 1 && !!v.branding && typeof v.branding.site_name === 'string' && !!v.branding.site_name.trim() && typeof v.branding.site_description === 'string' && image(v.branding.logo_url) && image(v.branding.favicon_url) && footer(v.branding.footer) && [v.login, v.oidc].every(a => a && image(a.cover_url) && typeof a.welcome_title === 'string' && typeof a.welcome_description === 'string')
}
