import { identityRequest } from '@/http/identity'
import type { Page } from '@/pages/account/api/type'
import type { Application, ApplicationsPage, Registration, RegistrationReceipt, ChangeReceipt, Reader, ReaderReceipt, Approval, Assignment, Delivery } from './type'
export async function getProviderMetadata(signal?: AbortSignal): Promise<{ issuer: string }> {
    const response = await fetch('/.well-known/openid-configuration', { signal, credentials: 'omit', cache: 'no-store' })
    if (!response.ok) throw new Error('无法读取身份提供方配置')
    const data = await response.json() as { issuer?: unknown }
    if (typeof data.issuer !== 'string') throw new Error('身份提供方配置缺少 Issuer')
    return { issuer: data.issuer }
}
export const listApps = (administrator: boolean, after?: number, signal?: AbortSignal) => identityRequest<ApplicationsPage>({ url: `${administrator ? 'admin' : 'developer'}/apps`, params: { after }, signal, errorPresentation: 'local' })
export const getApp = (administrator: boolean, id: number, signal?: AbortSignal) => identityRequest<Application>({ url: `${administrator ? 'admin' : 'developer'}/apps/${id}`, signal, errorPresentation: 'local' })
export const registerApp = (data: Registration, key: string) => identityRequest<RegistrationReceipt>({ url: 'developer/apps', method: 'POST', data, headers: { 'Idempotency-Key': key }, errorPresentation: 'local' })
export const registerAdminApp = (data: { application: Registration; owner_user_id: number; assignments: Assignment[]; accept_logout_limitation: boolean }, key: string) => identityRequest<RegistrationReceipt>({ url: 'admin/apps', method: 'POST', data, headers: { 'Idempotency-Key': key }, errorPresentation: 'local' })
export const changeApp = (administrator: boolean, id: number, path: string, method: 'POST' | 'PATCH' | 'PUT', data: unknown, key: string) => identityRequest<ChangeReceipt>({ url: `${administrator ? 'admin' : 'developer'}/apps/${id}${path}`, method, data, headers: { 'Idempotency-Key': key }, errorPresentation: 'local' })
export const getReaders = (administrator: boolean, id: number, env: string, signal?: AbortSignal, after?: string) => identityRequest<Page<Reader>>({ url: `${administrator ? 'admin' : 'developer'}/apps/${id}/environments/${encodeURIComponent(env)}/integration/readers`, params: { after }, signal, errorPresentation: 'local' })
export const changeReader = (administrator: boolean, id: number, env: string, readerId: string | null, data: unknown, key: string) => identityRequest<ReaderReceipt>({ url: `${administrator ? 'admin' : 'developer'}/apps/${id}/environments/${encodeURIComponent(env)}/integration/readers${readerId ? '/' + encodeURIComponent(readerId) : ''}`, method: readerId ? 'PATCH' : 'POST', data, headers: { 'Idempotency-Key': key }, errorPresentation: 'local' })
export const getApproval = (administrator: boolean, id: number, approvalId: string, signal?: AbortSignal) => identityRequest<Approval>({ url: `${administrator ? 'admin' : 'developer'}/apps/${id}/approvals/${encodeURIComponent(approvalId)}`, signal, errorPresentation: 'local' })
export const getCandidates = (signal?: AbortSignal, after?: number) => identityRequest<Page<{ user_id: number; username: string; nickname: string }, number>>({ url: 'developer/collaboration-candidates', params: { after }, signal, errorPresentation: 'local' })
export const searchCandidates = (q: string, signal?: AbortSignal, limit = 20) => identityRequest<Page<{ user_id: number; username: string; nickname: string }, number>>({ url: 'developer/collaboration-candidates', params: { q, limit }, signal, errorPresentation: 'local' })
export const getAppDeliveries = (administrator: boolean, id: number, env: string, signal?: AbortSignal, after?: string) => identityRequest<Page<Delivery>>({ url: `${administrator ? 'admin' : 'developer'}/apps/${id}/environments/${encodeURIComponent(env)}/logout-deliveries`, params: { after }, signal, errorPresentation: 'local' })
