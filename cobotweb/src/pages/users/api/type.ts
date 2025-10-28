import type { RoleVO, UserVO } from '@/types/models'
import type { PageResult } from '@/types/pagination'

export interface UserRolePatchReq {
  user_id: string
  new_role_id: string
}

export type UsersPage = PageResult<UserVO>
export type RolesPage = PageResult<RoleVO>

// 创建用户请求（用于注册或后台创建）
export interface UserCreateReq {
  username: string
  password: string
  nickname?: string
  email?: string
  role?: string
  status?: string
}