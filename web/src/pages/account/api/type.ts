export interface CenterSession {
    user_id: number
    sub: string
    username: string
    nickname: string | null
    email?: string
    email_verified?: boolean
    person_kind: 'employee' | 'collaborator'
    developer_enabled: boolean
    administrator: boolean
    sso_session_id: string
    auth_time: number
    expires_at: number
    csrf_token: string
}
export interface RevocationResult {
    revoked: boolean
}
export interface PasswordChangeResult { requires_login: boolean; user_id: number; oidc_flow_id: string | null }
export interface Page<T, C = string> { items: T[]; next: C | null }
export interface PersonalApplication { environment_id: string; application_id: number; name: string; environment: string; client_id: string; launchable?: boolean; launch_description?: string | null; sources?: { kind: 'direct' | 'group' | 'maintainer' | 'test_user'; source_id: string; label: string }[] }
export interface PersonalConsent { id: number; application_id: number; name: string; environment_id: string; environment: string; version_id: string; scopes: string[]; current_version: boolean }
export interface PersonalCenter { id: string; current: boolean; auth_time: number; expires_at: number; revoked_at: number | null }
export interface PersonalRp { id: string; center_id: string; name: string; environment: string; created_at: number; revoked_at: number | null; center_expires_at: number; center_revoked: boolean }
