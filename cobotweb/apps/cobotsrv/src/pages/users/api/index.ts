import { request } from '@/http/axios'
import type { UsersPage, RolesPage, UserRolePatchReq, UserCreateWithRoleReq, UserCreateResp, UserCreateReq } from './type'
import type { UserVO } from '@/types/models'
import type {
    UseAuthorityAddReq,
    UseAuthorityDeleteReq,
    UseAuthorityPatchReq
} from "./type"
import type { UserAuthorityVO } from "@/types/models"
import type { PageResult } from '@/types/pagination'

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

export async function deleteUser(id: number): Promise<string> {
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




// ======== 用户权限覆写接口 ========
// 分页获取用户权限覆写列表
export async function getUserAuthorityOverridesByPage(page_number: number, page_size: number): Promise<PageResult<UserAuthorityVO>> {
    return request<PageResult<UserAuthorityVO>>({
        url: "/user_authority_override",
        method: "GET",
        params: { page_number, page_size }
    })
}

// 新增用户权限覆写
export async function addUserAuthorityOverride(payload: UseAuthorityAddReq): Promise<UserAuthorityVO>
{
    return request<UserAuthorityVO>({
        url: "/user_authority_override",
        method: "POST",
        data: payload
    })
}

// 删除用户权限覆写
export async function deleteUserAuthorityOverride(payload: UseAuthorityDeleteReq): Promise<UserAuthorityVO>
{
    return request<UserAuthorityVO>({
        url: "/user_authority_override",
        method: "DELETE",
        data: payload
    })
}

// 更新用户权限覆写（仅 allow）
export async function patchUserAuthorityOverride(payload: UseAuthorityPatchReq): Promise<UserAuthorityVO>
{
    return request<UserAuthorityVO>({
        url: "/user_authority_override",
        method: "PATCH",
        data: payload
    })
}

// 注意：不要添加新的后端接口，这里保留通用分页查询，界面按 user_id 进行本地过滤
