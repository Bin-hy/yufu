import { identityRequest } from '@/http/identity'
import type { SettingsResponse, SiteSettingsDocument, AssetKind, AssetMetadata } from '@/branding/types'
const options = { errorPresentation: 'local' as const }
export const getSiteSettings = (signal?: AbortSignal) => identityRequest<SettingsResponse>({ ...options, url: 'admin/site-settings', signal })
export const saveSiteSettings = (document: SiteSettingsDocument, expected_revision: number) => identityRequest<SettingsResponse>({ ...options, url: 'admin/site-settings', method: 'PUT', data: { document, expected_revision } })
export const resetSiteSettings = (expected_revision: number) => identityRequest<SettingsResponse>({ ...options, url: 'admin/site-settings/reset', method: 'POST', data: { expected_revision } })
export const uploadSiteAsset = (kind: AssetKind, file: File, signal?: AbortSignal) => identityRequest<AssetMetadata>({ ...options, url: 'admin/site-assets', method: 'POST', params: { kind }, data: file, headers: { 'Content-Type': 'application/octet-stream' }, timeout: 60000, signal })
export const readSiteAsset = (id: string, signal?: AbortSignal) => identityRequest<Blob>({ ...options, url: `admin/site-assets/${encodeURIComponent(id)}`, responseType: 'blob', signal })
