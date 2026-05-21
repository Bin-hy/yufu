import { request } from '@/http/axios'
import type { RoleVO } from '@/types/models'
import type { RolesPage, RoleAddReq, RolePatchReq, AuthRoleAddReq, AuthRoleDeleteReq } from './type'

export async function getRolesByPage(page_number: number, page_size: number): Promise<RolesPage> {
  return await request<RolesPage>({
    url: '/role',
    method: 'GET',
    params: { page_number, page_size }
  })
}

export async function addRole(payload: RoleAddReq): Promise<RoleVO> {
  return await request<RoleVO>({
    url: '/role',
    method: 'POST',
    data: payload
  })
}

export async function patchRole(payload: RolePatchReq): Promise<RoleVO> {
  return await request<RoleVO>({
    url: '/role',
    method: 'PATCH',
    data: payload
  })
}

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
