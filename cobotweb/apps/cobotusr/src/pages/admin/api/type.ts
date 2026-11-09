import type { Registration, Assignment } from '@/pages/developer/api/type'
export interface EnterpriseAccount { user_id: number; username: string; nickname: string; enabled: boolean; person_kind: string; developer_enabled: boolean; revision: number }
export interface Group { group_id: string; name: string; description: string | null; revision: number; members: number[]; application_ids: number[] }
export interface Audit { id: string; request_id: string; actor_id: number | null; action: string; object_kind: string; object_id: string; happened_at: number; result: string; reason_code: string | null; change_summary: Record<string, unknown> }
export interface Delivery { id: string; event_id: string; application_id: number; name: string; environment: string; status: string; attempt_count: number; next_attempt_at: number; last_error_code: string | null; created_at: number; delivered_at: number | null }
export interface IssuedAction { id: string; secret: string; expires_at: number }
export interface LegacyClient { oauth_client_id: number; client_id: string; name: string; candidate_owner_id: number; public_client: boolean; management_state: string; candidate_application_ids: number[]; conflict_kinds: string[]; snapshot: string }
export interface Adoption { oauth_client_id: number; expected_snapshot: string; existing_application_id: number | null; retained_environment: 'development' | 'production'; registration: { application: Registration; owner_user_id: number; assignments: Assignment[]; accept_logout_limitation: boolean } }
