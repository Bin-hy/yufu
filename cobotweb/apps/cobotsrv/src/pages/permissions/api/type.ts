import type { PageResult } from '@/types/pagination'
import type { PermissionVO } from '@/types/models'

export type PermissionPage = PageResult<PermissionVO>

export interface PermissionInsertReq {
  application_id: number
  name: string
  description?: string | null
  action: string
  resource_type?: string | null
  resource_pattern?: string | null
}

export interface PermissionPatchReq {
  id: number
  name?: string | null
  description?: string | null
  action?: string | null
  resource_type?: string | null
  resource_pattern?: string | null
  application_id?: number | null
}

export interface PermissionDeleteReq {
  id: number
}

// role_permission types
export interface RolePermAddReq {
  role_id: number
  permission_id: number
}

export interface RolePermBatchAddReq {
  role_id: number
  permission_ids: number[]
}

export interface RolePermDeleteReq {
  role_id: number
  permission_id: number
}

export interface RolePermBatchDeleteReq {
  role_id: number
  permission_ids: number[]
}
