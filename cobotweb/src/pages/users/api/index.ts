import { request } from '@/http/axios'
import type { UsersPage, RolesPage, UserRolePatchReq, UserCreateReq } from './type'
import type { RoleVO, UserVO } from '@/types/models'

export async function getUsersByPage(page_number: number, page_size: number): Promise<UsersPage> {
  const res = await request<{ data: UsersPage }>({
    url: '/users',
    method: 'GET',
    params: { page_number, page_size }
  })
  return res.data
}

export async function getModifiableRoles(page_number = 1, page_size = 100): Promise<RolesPage> {
  const res = await request<{ data: RolesPage }>({
    url: '/role/modifiable',
    method: 'GET',
    params: { page_number, page_size }
  })
  return res.data
}

export async function patchUserRole(payload: UserRolePatchReq): Promise<UserVO> {
  const res = await request<{ data: UserVO }>({
    url: '/user/role',
    method: 'PATCH',
    data: payload
  })
  return res.data
}

export async function deleteRole(id: string): Promise<string> {
  const res = await request<{ data: string }>({
    url: '/role',
    method: 'DELETE',
    data: { id }
  })
  return res.data
}

// 创建用户（用于注册或后台创建），与后端保持 RESTful 语义：POST /users
export async function createUser(payload: UserCreateReq): Promise<UserVO> {
  const res = await request<{ data: UserVO }>({
    url: '/users',
    method: 'POST',
    data: payload
  })
  return res.data
}