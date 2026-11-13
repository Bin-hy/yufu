import { identityRequest } from '@/http/identity'
import type { RequestOptions } from '@/http/axios'
import type { CenterSession, RevocationResult, PasswordChangeResult } from './type'
import type { Page, PersonalApplication, PersonalConsent, PersonalCenter, PersonalRp } from './type'
export const getApplications = (after?: string, signal?: AbortSignal, environment: 'production' | 'development' = 'production', q?: string) => identityRequest<Page<PersonalApplication>>({ url: 'account/apps', params: { after, environment, q }, signal, errorPresentation: 'local' })
export const getConsents = (after?: number, signal?: AbortSignal) => identityRequest<Page<PersonalConsent, number>>({ url: 'account/consents', params: { after }, signal, errorPresentation: 'local' })
export const getCenters = (after?: string, signal?: AbortSignal) => identityRequest<Page<PersonalCenter>>({ url: 'account/sessions', params: { after }, signal, errorPresentation: 'local' })
export const getRps = (after?: string, signal?: AbortSignal) => identityRequest<Page<PersonalRp>>({ url: 'account/rp-sessions', params: { after }, signal, errorPresentation: 'local' })
export const revokeConsent = (id: number) => identityRequest<RevocationResult>({ url: `account/consents/${id}/revoke`, method: 'POST', errorPresentation: 'local' })
export const revokeCenter = (id: string) => identityRequest<RevocationResult>({ url: `account/sessions/${encodeURIComponent(id)}/revoke`, method: 'POST', errorPresentation: 'local' })
export const revokeRp = (id: string) => identityRequest<RevocationResult>({ url: `account/rp-sessions/${encodeURIComponent(id)}/revoke`, method: 'POST', errorPresentation: 'local' })

export const getCenterSession = (options: RequestOptions = {}) =>
    identityRequest<CenterSession>({ ...options, url: 'account/session' })
export const centerLogin = (data: { username: string; password: string; flow_id?: string }) =>
    identityRequest<CenterSession>({ url: 'identity/login', method: 'POST', data, errorPresentation: 'local' })
export const centerLogout = (all = false) =>
    identityRequest<RevocationResult>({ url: all ? 'account/logout-all' : 'identity/logout', method: 'POST', errorPresentation: 'local' })
export const updateProfile = (data: { nickname: string; email: string | null }) =>
    identityRequest<{ username: string; nickname: string; email?: string }>({ url: 'account/profile', method: 'PATCH', data, errorPresentation: 'local' })
export const changePassword = (data: { old_password: string; new_password: string }) =>
    identityRequest<PasswordChangeResult>({ url: 'account/password', method: 'POST', data, errorPresentation: 'local' })
