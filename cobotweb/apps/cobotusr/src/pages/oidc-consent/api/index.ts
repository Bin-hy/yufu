import { request } from '@/http/axios'
import type { RequestOptions } from '@/http/axios'
import type { ClientInfoResponse, ConsentRequest, ConsentResponse } from './type'

export async function getAuthorizeClientInfo(
    params: {
        client_id: string
        scope?: string
    },
    options: RequestOptions = {},
): Promise<ClientInfoResponse> {
    return request<ClientInfoResponse>({
        url: '/oidc/authorize/client-info',
        method: 'GET',
        params,
        ...options,
    })
}

export async function submitConsent(
    data: ConsentRequest,
    options: RequestOptions = {},
): Promise<ConsentResponse> {
    return request<ConsentResponse>({
        url: '/oidc/authorize/consent',
        method: 'POST',
        data,
        ...options,
    })
}
