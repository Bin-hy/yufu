import { identityRequest } from '@/http/identity'

export interface ExternalProviderAdminView {
    provider_key: string
    family: string
    enabled: boolean
    client_id: string
    secret_configured: boolean
    config_complete: boolean
    revision: number
    updated_at: number
    updated_by: number | null
}

export interface ProviderStat {
    provider_key: string
    success: number
    denied: number
    failure: number
    reasons: [string, number][]
}

export interface ExternalProviderStats {
    from: number
    to: number
    providers: ProviderStat[]
}

export interface UpdateExternalProvider {
    expected_revision: number
    enabled: boolean
    client_id: string
    secret?: string
}

const options = { errorPresentation: 'local' as const }

export const getExternalProvidersAdmin = (signal?: AbortSignal) =>
    identityRequest<ExternalProviderAdminView[]>({ ...options, url: 'admin/external-providers', signal })

export const updateExternalProvider = (key: string, data: UpdateExternalProvider) =>
    identityRequest<ExternalProviderAdminView>({
        ...options,
        url: `admin/external-providers/${encodeURIComponent(key)}`,
        method: 'PUT',
        data,
    })

export const rotateExternalProviderSecret = (key: string, expected_revision: number, secret: string) =>
    identityRequest<ExternalProviderAdminView>({
        ...options,
        url: `admin/external-providers/${encodeURIComponent(key)}/secret`,
        method: 'POST',
        data: { expected_revision, secret },
    })

export const getExternalProviderStats = (window: '24h' | '7d', signal?: AbortSignal) =>
    identityRequest<ExternalProviderStats>({
        ...options,
        url: 'admin/external-providers/stats',
        params: { window },
        signal,
    })
