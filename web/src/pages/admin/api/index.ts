import { identityRequest } from '@/http/identity'
import type { Page } from '@/pages/account/api/type'
import type { EnterpriseAccount, Group, Audit, Delivery, IssuedAction, LegacyClient, Adoption } from './type'
import type { ChangeReceipt, Assignment, AssignmentSummary, RegistrationReceipt } from '@/pages/developer/api/type'
const options = { errorPresentation: 'local' as const }
export const listAccounts = (after?: number, signal?: AbortSignal, q?: string) => identityRequest<Page<EnterpriseAccount, number>>({ ...options, url: 'admin/accounts', params: { after, q, limit: 25 }, signal })
export const searchAccounts = (q: string, signal?: AbortSignal, limit = 20) => identityRequest<Page<EnterpriseAccount, number>>({ ...options, url: 'admin/accounts', params: { q, limit }, signal })
export const setAccount = (id: number, field: 'enabled' | 'developer', value: boolean, revision: number) => identityRequest<EnterpriseAccount>({ ...options, url: `admin/accounts/${id}/${field}`, method: 'PATCH', data: { [field === 'developer' ? 'developer_enabled' : 'enabled']: value, expected_revision: revision } })
export const issueInvitation = (data: { purpose: 'onboarding' | 'qualification'; target_user_id: number | null; person_kind: 'employee' | 'collaborator'; developer_grant: boolean }) => identityRequest<IssuedAction>({ ...options, url: 'admin/invitations', method: 'POST', data })
export const issueReset = (user_id: number) => identityRequest<IssuedAction>({ ...options, url: 'admin/password-resets', method: 'POST', data: { user_id } })
export const revokeAction = (id: string, reset: boolean) => identityRequest<{ revoked: boolean }>({ ...options, url: `admin/${reset ? 'password-resets' : 'invitations'}/${encodeURIComponent(id)}/revoke`, method: 'POST' })
export const listGroups = (after?: string, signal?: AbortSignal, q?: string) => identityRequest<Page<Group>>({ ...options, url: 'admin/groups', params: { after, q, limit: 25 }, signal })
export const searchGroups = (q: string, signal?: AbortSignal, limit = 20) => identityRequest<Page<Group>>({ ...options, url: 'admin/groups', params: { q, limit }, signal })
export const groupChange = (path: string, method: 'POST' | 'PATCH' | 'DELETE', data: unknown, key: string) => identityRequest<ChangeReceipt>({ ...options, url: `admin/groups${path}`, method, data, headers: { 'Idempotency-Key': key } })
export const listAudit = (after?: string, signal?: AbortSignal) => identityRequest<Page<Audit>>({ ...options, url: 'admin/audit-events', params: { after }, signal })
export const listDeliveries = (after?: string, signal?: AbortSignal) => identityRequest<Page<Delivery>>({ ...options, url: 'admin/logout-deliveries', params: { after }, signal })
export const retryDelivery = (id: string) => identityRequest<{ queued: boolean }>({ ...options, url: `admin/logout-deliveries/${encodeURIComponent(id)}/retry`, method: 'POST' })
export const getAssignments = (id: number, signal?: AbortSignal) => identityRequest<AssignmentSummary[]>({ ...options, url: `admin/apps/${id}/assignments`, signal })
export const setAssignment = (id: number, target: Assignment, present: boolean, expected_revision: number, key: string) => identityRequest<ChangeReceipt>({ ...options, url: `admin/apps/${id}/assignments`, method: 'PATCH', data: { target, present, expected_revision }, headers: { 'Idempotency-Key': key } })
export const setTestUser = (id: number, env: string, user_id: number, present: boolean, expected_revision: number, key: string) => identityRequest<ChangeReceipt>({ ...options, url: `admin/apps/${id}/environments/${encodeURIComponent(env)}/test-users/${user_id}`, method: 'PATCH', data: { present, expected_revision }, headers: { 'Idempotency-Key': key } })
export const setCandidate = (requester_user_id: number, candidate_user_id: number, allowed: boolean) => identityRequest<{ allowed: boolean }>({ ...options, url: 'admin/collaboration-candidates', method: 'PATCH', data: { requester_user_id, candidate_user_id, allowed } })
export const listMigrations = (after?: number, signal?: AbortSignal) => identityRequest<Page<LegacyClient, number>>({ ...options, url: 'admin/identity/migrations', params: { after }, signal })
export const scanMigrations = (after = 0) => identityRequest<Page<LegacyClient, number>>({ ...options, url: 'admin/identity/migrations/scan', method: 'POST', data: { after, limit: 20 } })
export const adoptLegacy = (data: Adoption, key: string) => identityRequest<RegistrationReceipt>({ ...options, url: `admin/identity/migrations/${data.oauth_client_id}/adopt`, method: 'POST', data, headers: { 'Idempotency-Key': key } })
