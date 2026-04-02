import { request } from '@/http/axios'
import type {
  ClientVO,
  ClientCreateReq,
  ClientCreateResp,
  ClientUpdateReq,
  SecretResp,
  OAuthScopeVO,
  ScopeCreateReq
} from './type'

// --- Client ---

export async function listClients(): Promise<ClientVO[]> {
  return request<ClientVO[]>({
    url: '/oidc/clients',
    method: 'GET'
  })
}

export async function getClient(id: number): Promise<ClientVO> {
  return request<ClientVO>({
    url: `/oidc/client/${id}`,
    method: 'GET'
  })
}

export async function createClient(payload: ClientCreateReq): Promise<ClientCreateResp> {
  return request<ClientCreateResp>({
    url: '/oidc/clients',
    method: 'POST',
    data: payload
  })
}

export async function updateClient(id: number, payload: ClientUpdateReq): Promise<ClientVO> {
  return request<ClientVO>({
    url: `/oidc/client/${id}`,
    method: 'PATCH',
    data: payload
  })
}

export async function deleteClient(id: number): Promise<void> {
  return request<void>({
    url: `/oidc/client/${id}`,
    method: 'DELETE'
  })
}

export async function regenerateSecret(id: number): Promise<SecretResp> {
  return request<SecretResp>({
    url: `/oidc/client/${id}/secret`,
    method: 'POST'
  })
}

// --- Scope ---

export async function listScopes(): Promise<OAuthScopeVO[]> {
  return request<OAuthScopeVO[]>({
    url: '/oidc/scope',
    method: 'GET'
  })
}

export async function createScope(payload: ScopeCreateReq): Promise<OAuthScopeVO> {
  return request<OAuthScopeVO>({
    url: '/oidc/scope',
    method: 'POST',
    data: payload
  })
}
