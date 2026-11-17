import { validatePublicBranding } from './resolve'
export async function loadPublicBranding(signal?: AbortSignal) {
    const response = await fetch('/api/v1/site/branding', { credentials: 'omit', cache: 'no-store', signal })
    if (!response.ok) throw new Error('站点配置暂不可用')
    const data: unknown = await response.json()
    if (!validatePublicBranding(data)) throw new Error('站点配置格式无效')
    return data
}
