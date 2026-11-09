import { identityRequest } from '@/http/identity'
import type { PasswordChangeResult } from './type'
export interface ActionView { action_id: string; kind: 'invitation' | 'password_reset'; expires_at: number; person_kind: 'employee' | 'collaborator' | null; developer_grant: boolean; requires_existing_account: boolean; oidc_flow_id: string | null; csrf_token: string }
export const exchangeAction = (data: { kind: string; secret: string; flow_id?: string }) => identityRequest<{ action_id: string; expires_at: number; oidc_flow_id: string | null }>({ url: 'identity/actions/exchange', method: 'POST', data, errorPresentation: 'local' })
export const inspectAction = (id: string, signal?: AbortSignal) => identityRequest<ActionView>({ url: `identity/actions/${encodeURIComponent(id)}`, signal, errorPresentation: 'local' })
export const acceptInvitation = (data: { action_id: string; existing_account: boolean; username?: string; password?: string; nickname?: string }) => identityRequest<PasswordChangeResult>({ url: 'identity/invitations/accept', method: 'POST', data, errorPresentation: 'local' })
export const completePasswordReset = (action_id: string, new_password: string) => identityRequest<PasswordChangeResult>({ url: 'identity/password-resets/complete', method: 'POST', data: { action_id, new_password }, errorPresentation: 'local' })

export function consumeSecretFragment(location: Pick<Location, 'hash' | 'pathname' | 'search'>, history: Pick<History, 'replaceState' | 'state'>) {
    const fragmentIndex = location.hash.lastIndexOf('#')
    const fragment = location.hash.slice(fragmentIndex + 1)
    const params = new URLSearchParams(fragment)
    if (!params.has('secret')) return null
    // Erase before any exchange request, also preserving a hash-router path if present.
    history.replaceState(history.state, '', location.pathname + location.search + location.hash.slice(0, fragmentIndex))
    const secret = params.get('secret') ?? '', kind = params.get('kind') ?? 'invitation'
    if (!/^[A-Za-z0-9_-]{43}$/.test(secret) || !['invitation', 'password_reset'].includes(kind)) throw new Error('账户操作链接无效，请重新打开管理员提供的链接')
    return { kind, secret }
}
