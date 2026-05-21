import { request } from '@/http/axios'
import type { ClientInfoResponse, ConsentRequest, ConsentResponse } from './type'

export async function getAuthorizeClientInfo(params: {
  client_id: string
  scope?: string
}): Promise<ClientInfoResponse> {
  return request<ClientInfoResponse>({
    url: '/oidc/authorize/client-info',
    method: 'GET',
    params,
  })
}

export async function submitConsent(data: ConsentRequest): Promise<ConsentResponse> {
  return request<ConsentResponse>({
    url: '/oidc/authorize/consent',
    method: 'POST',
    data,
  })
}
