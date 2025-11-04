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
    nickname: string
    password: string
}

export interface UserCreateWithRoleReq {
    username: string
    nickname: string
    password: string
    role_id: string
}

// 创建成功响应的数据
export interface UserCreateResp
{
    create_time: string,
    id: string,
    nickname: string,
    role: RoleVO,
    role_id: string,
    update_time: string,
    username: string
}
