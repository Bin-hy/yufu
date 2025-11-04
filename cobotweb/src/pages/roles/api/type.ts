import type { PageResult } from '@/types/pagination'
import type { RoleVO } from '@/types/models'

// 分页结果
export type RolesPage = PageResult<RoleVO>

// 创建角色请求体
export interface RoleAddReq {
  name: string
  role_enum: string
}

// 重命名/更新角色请求体
export interface RolePatchReq {
  id: string
  name: string
  role_enum: string
}

// 角色-权限 关联请求体
export interface AuthRoleAddReq { role_id: string; authority_id: string }
export interface AuthRoleDeleteReq { role_id: string; authority_id: string }