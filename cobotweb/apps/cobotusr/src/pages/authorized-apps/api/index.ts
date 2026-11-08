import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type { AuthorizedAppVO } from './type'

export async function listAuthorizedApps(options: RequestOptions = {}): Promise<AuthorizedAppVO[]> {
    return request<AuthorizedAppVO[]>({
        ...options,
        url: '/oidc/authorized-apps',
        method: 'GET',
    })
}

export async function revokeAuthorizedApp(
    consentId: number,
    options: RequestOptions = {},
): Promise<void> {
    return request<void>({
        ...options,
        url: `/oidc/authorized-apps/${consentId}`,
        method: 'DELETE',
    })
}
