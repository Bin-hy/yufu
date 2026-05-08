import { request } from '@/http/axios'
import type { AuthorizedAppVO } from './type'

export async function listAuthorizedApps(): Promise<AuthorizedAppVO[]> {
  return request<AuthorizedAppVO[]>({
    url: '/oidc/authorized-apps',
    method: 'GET',
  })
}

export async function revokeAuthorizedApp(consentId: number): Promise<void> {
  return request<void>({
    url: `/oidc/authorized-apps/${consentId}`,
    method: 'DELETE',
  })
}
