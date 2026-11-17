export type AssetKind = 'logo' | 'favicon' | 'cover'
export interface FooterLink { label: string; url: string }
export interface FooterConfig { copyright_text: string; filing_number: string; filing_url: string | null; links: FooterLink[] }
export interface AuthAppearance { cover_url: string | null; welcome_title: string; welcome_description: string }
export interface SiteSettingsDocument {
    branding: {
        site_name: string; site_description: string
        logo_asset_id: string | null; favicon_asset_id: string | null; cover_asset_id: string | null
        welcome_title: string; welcome_description: string; footer: FooterConfig
    }
    oidc: { cover_asset_id: string | null; welcome_title: string | null; welcome_description: string | null }
}
export interface PublicSiteBranding {
    revision: number
    branding: { site_name: string; site_description: string; logo_url: string | null; favicon_url: string | null; footer: FooterConfig }
    login: AuthAppearance; oidc: AuthAppearance
}
export interface SiteSettings { document: SiteSettingsDocument; revision: number; updated_by: number | null; updated_at: number | null }
export interface SettingsResponse { settings: SiteSettings; limits: Record<string, number> }
export interface AssetMetadata { id: string; kind: AssetKind; mime: string; width: number; height: number; byte_size: number; sha256: string; preview_url: string }
