import { identityRequest } from '@/http/identity'

export interface ExternalProvider {
    key: string
    family: string
    label: string
    mode: 'oauth' | 'native'
    enabled: boolean
}

export interface ExternalIdentity {
    id: string
    provider_key: string
    provider_label: string
    display_name: string | null
    subject_hint: string
    created_at: number
    last_login_at: number | null
}

export const getExternalProviders = (signal?: AbortSignal) =>
    identityRequest<ExternalProvider[]>({ url: 'identity/external/providers', signal, errorPresentation: 'local' })

export const startExternalLogin = (key: string, flow_id?: string) =>
    identityRequest<{ authorize_url: string }>({
        url: `identity/external/${encodeURIComponent(key)}/start`,
        method: 'POST',
        data: { flow_id },
        errorPresentation: 'local',
    })

export const startExternalBind = (key: string) =>
    identityRequest<{ authorize_url: string }>({
        url: `identity/external/${encodeURIComponent(key)}/bind/start`,
        method: 'POST',
        errorPresentation: 'local',
    })

export const getExternalIdentities = (signal?: AbortSignal) =>
    identityRequest<ExternalIdentity[]>({ url: 'account/identities', signal, errorPresentation: 'local' })

export const unbindExternalIdentity = (id: string) =>
    identityRequest<{ unbound: boolean }>({
        url: `account/identities/${encodeURIComponent(id)}/unbind`,
        method: 'POST',
        errorPresentation: 'local',
    })
