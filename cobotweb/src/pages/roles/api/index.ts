import { request } from '@/http/axios'
import type { PageResult } from '@/types/pagination'
import type { RoleVO } from '@/types/models'

export type RolesPage = PageResult<RoleVO>

export async function getRolesByPage(page_number: number, page_size: number): Promise<RolesPage> {
  const res = await request<{ data: RolesPage }>({
    url: '/role',
    method: 'GET',
    params: { page_number, page_size }
  })
  return res.data
}

// 创建角色
export interface RoleAddReq {
  name: string
  role_enum: string
}

export async function addRole(payload: RoleAddReq): Promise<RoleVO> {
  const res = await request<{ data: RoleVO }>({
    url: '/role',
    method: 'POST',
    data: payload
  })
  return res.data
}

// 重命名角色（保留 role_enum，不建议前端修改枚举）
export interface RolePatchReq {
  id: string
  name: string
  role_enum: string
}

export async function patchRole(payload: RolePatchReq): Promise<RoleVO> {
  const res = await request<{ data: RoleVO }>({
    url: '/role',
    method: 'PATCH',
    data: payload
  })
  return res.data
}

// 角色权限增删
export interface AuthRoleAddReq { role_id: string; authority_id: string }
export interface AuthRoleDeleteReq { role_id: string; authority_id: string }

export async function addRoleAuthority(payload: AuthRoleAddReq): Promise<boolean> {
  try {
    await request<{ data: any }>({
      url: '/role_authority',
      method: 'POST',
      data: payload
    })
    return true
  } catch (e) {
    // Axios 拦截器已统一弹错；409 视作已存在不算失败
    return false
  }
}

export async function deleteRoleAuthority(payload: AuthRoleDeleteReq): Promise<boolean> {
  try {
    await request<{ data: string }>({
      url: '/role_authority',
      method: 'DELETE',
      data: payload
    })
    return true
  } catch (e) {
    return false
  }
}