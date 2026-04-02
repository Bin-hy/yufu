import type { PageResult } from '@/types/pagination'
import type { RoleVO } from '@/types/models'

// 分页结果
export type RolesPage = PageResult<RoleVO>

// 创建角色请求体
export interface RoleAddReq {
  name: string
}

// 重命名/更新角色请求体
export interface RolePatchReq {
  id: number
  name: string
}

// 角色-权限 关联请求体
export interface AuthRoleAddReq { role_id: number; authority_id: number }
export interface AuthRoleDeleteReq { role_id: number; authority_id: number }
