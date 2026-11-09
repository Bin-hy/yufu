import { identityRequest } from '@/http/identity'
import type { AuthorizationFlow } from './type'
export const inspectFlow = (id: string, signal?: AbortSignal) =>
    identityRequest<AuthorizationFlow>({ url: `identity/flows/${encodeURIComponent(id)}`, signal, errorPresentation: 'local' })
export const completeFlow = (id: string, approved: boolean) =>
    identityRequest<{ redirect_url: string }>({ url: `identity/flows/${encodeURIComponent(id)}/complete`, method: 'POST', data: { approved }, errorPresentation: 'local' })
