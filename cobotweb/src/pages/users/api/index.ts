import { request } from '@/http/axios'
import type { UsersPage, RolesPage, UserRolePatchReq, UserCreateWithRoleReq, UserCreateResp, UserCreateReq } from './type'
import type { UserVO } from '@/types/models'

export async function getUsersByPage(page_number: number, page_size: number): Promise<UsersPage> {
  return request<UsersPage>({
    url: '/users',
    method: 'GET',
    params: { page_number, page_size }
  })
}

export async function getModifiableRoles(page_number = 1, page_size = 100): Promise<RolesPage> {
  return request<RolesPage>({
    url: '/role/modifiable',
    method: 'GET',
    params: { page_number, page_size }
  })
}

export async function patchUserRole(payload: UserRolePatchReq): Promise<UserVO> {
  return request<UserVO>({
    url: '/user/role',
    method: 'PATCH',
    data: payload
  })
}

export async function deleteUser(id: string): Promise<string> {
  return request<string>({
    url: `/user/${id}`,
    method: 'DELETE',
  })
}

// 创建用户（用于注册或后台创建），与后端保持 RESTful 语义：POST /user
export async function createUser(payload: UserCreateReq): Promise<UserCreateResp> {
  return request<UserCreateResp>({
    url: '/register',
    method: 'POST',
    data: payload
  })
}

// 创建用户（用于注册或后台创建），与后端保持 RESTful 语义：POST /user
export async function createUserWithRole(payload: UserCreateWithRoleReq): Promise<UserCreateResp> {
  return request<UserCreateResp>({
    url: '/user',
    method: 'POST',
    data: payload
  })
}
