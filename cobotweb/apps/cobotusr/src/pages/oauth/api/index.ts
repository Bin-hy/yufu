import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type {
    ClientVO,
    ClientCreateReq,
    ClientCreateResp,
    ClientUpdateReq,
    SecretResp,
    OAuthScopeVO,
    ScopeCreateReq,
} from './type'

// --- Client ---

export async function listClients(options: RequestOptions = {}): Promise<ClientVO[]> {
    return request<ClientVO[]>({
        ...options,
        url: '/oidc/clients',
        method: 'GET',
    })
}

export async function getClient(id: number, options: RequestOptions = {}): Promise<ClientVO> {
    return request<ClientVO>({
        ...options,
        url: `/oidc/client/${id}`,
        method: 'GET',
    })
}

export async function createClient(
    payload: ClientCreateReq,
    options: RequestOptions = {},
): Promise<ClientCreateResp> {
    return request<ClientCreateResp>({
        ...options,
        url: '/oidc/clients',
        method: 'POST',
        data: payload,
    })
}

export async function updateClient(
    id: number,
    payload: ClientUpdateReq,
    options: RequestOptions = {},
): Promise<ClientVO> {
    return request<ClientVO>({
        ...options,
        url: `/oidc/client/${id}`,
        method: 'PATCH',
        data: payload,
    })
}

export async function deleteClient(id: number, options: RequestOptions = {}): Promise<void> {
    return request<void>({
        ...options,
        url: `/oidc/client/${id}`,
        method: 'DELETE',
    })
}

export async function regenerateSecret(
    id: number,
    options: RequestOptions = {},
): Promise<SecretResp> {
    return request<SecretResp>({
        ...options,
        url: `/oidc/client/${id}/secret`,
        method: 'POST',
    })
}

// --- Scope ---

export async function listScopes(options: RequestOptions = {}): Promise<OAuthScopeVO[]> {
    return request<OAuthScopeVO[]>({
        ...options,
        url: '/oidc/scope',
        method: 'GET',
    })
}

export async function createScope(
    payload: ScopeCreateReq,
    options: RequestOptions = {},
): Promise<OAuthScopeVO> {
    return request<OAuthScopeVO>({
        ...options,
        url: '/oidc/scope',
        method: 'POST',
        data: payload,
    })
}
