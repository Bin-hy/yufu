import type { RoleVO, UserVO } from '@/types/models'
import type { PageResult } from '@/types/pagination'

export interface UserRolePatchReq {
  user_id: number
  new_role_id: number
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
    role_id: number
}

// 创建成功响应的数据
export interface UserCreateResp
{
    create_time: string,
    id: number,
    nickname: string,
    role: RoleVO,
    role_id: number,
    update_time: string,
    username: string
}

// ==============================
// 用户权限覆写请求
export interface UseAuthorityAddReq{
    user_id: number,
    authority_id: number,
    allow: boolean,
}

export interface UseAuthorityDeleteReq{
    user_id: number,
    authority_id: number,
    allow: boolean,
}

export interface UseAuthorityPatchReq{
    user_id: number,
    authority_id: number,
    allow: boolean,
}
