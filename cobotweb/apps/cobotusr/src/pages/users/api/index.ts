import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type {
    UsersPage,
    RolesPage,
    UserRolePatchReq,
    UserCreateWithRoleReq,
    UserCreateResp,
    UserCreateReq,
} from './type'
import type { UserVO } from '@/types/models'
import type { UseAuthorityAddReq, UseAuthorityDeleteReq, UseAuthorityPatchReq } from './type'
import type { UserAuthorityVO } from '@/types/models'
import type { PageResult } from '@/types/pagination'

export async function getUsersByPage(
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
): Promise<UsersPage> {
    return request<UsersPage>({
        ...options,
        url: '/users',
        method: 'GET',
        params: { page_number, page_size },
    })
}

export async function getModifiableRoles(
    page_number = 1,
    page_size = 100,
    options: RequestOptions = {},
): Promise<RolesPage> {
    return request<RolesPage>({
        ...options,
        url: '/role/modifiable',
        method: 'GET',
        params: { page_number, page_size },
    })
}

export async function patchUserRole(
    payload: UserRolePatchReq,
    options: RequestOptions = {},
): Promise<UserVO> {
    return request<UserVO>({
        ...options,
        url: '/user/role',
        method: 'PATCH',
        data: payload,
    })
}

export async function deleteUser(id: number, options: RequestOptions = {}): Promise<string> {
    return request<string>({
        ...options,
        url: `/user/${id}`,
        method: 'DELETE',
    })
}

// 创建用户（用于注册或后台创建），与后端保持 RESTful 语义：POST /user
export async function createUser(
    payload: UserCreateReq,
    options: RequestOptions = {},
): Promise<UserCreateResp> {
    return request<UserCreateResp>({
        ...options,
        url: '/register',
        method: 'POST',
        data: payload,
    })
}

// 创建用户（用于注册或后台创建），与后端保持 RESTful 语义：POST /user
export async function createUserWithRole(
    payload: UserCreateWithRoleReq,
    options: RequestOptions = {},
): Promise<UserCreateResp> {
    return request<UserCreateResp>({
        ...options,
        url: '/user',
        method: 'POST',
        data: payload,
    })
}

// ======== 用户权限覆写接口 ========
// 分页获取用户权限覆写列表
export async function getUserAuthorityOverridesByPage(
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
): Promise<PageResult<UserAuthorityVO>> {
    return request<PageResult<UserAuthorityVO>>({
        ...options,
        url: '/user_authority_override',
        method: 'GET',
        params: { page_number, page_size },
    })
}

// 新增用户权限覆写
export async function addUserAuthorityOverride(
    payload: UseAuthorityAddReq,
    options: RequestOptions = {},
): Promise<UserAuthorityVO> {
    return request<UserAuthorityVO>({
        ...options,
        url: '/user_authority_override',
        method: 'POST',
        data: payload,
    })
}

// 删除用户权限覆写
export async function deleteUserAuthorityOverride(
    payload: UseAuthorityDeleteReq,
    options: RequestOptions = {},
): Promise<UserAuthorityVO> {
    return request<UserAuthorityVO>({
        ...options,
        url: '/user_authority_override',
        method: 'DELETE',
        data: payload,
    })
}

// 更新用户权限覆写（仅 allow）
export async function patchUserAuthorityOverride(
    payload: UseAuthorityPatchReq,
    options: RequestOptions = {},
): Promise<UserAuthorityVO> {
    return request<UserAuthorityVO>({
        ...options,
        url: '/user_authority_override',
        method: 'PATCH',
        data: payload,
    })
}

// ======== 用户资源权限覆写接口 ========
export async function getUserPermissionOverridesByPage(
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
): Promise<PageResult<import('@/types/models').UserPermVO>> {
    return request<PageResult<import('@/types/models').UserPermVO>>({
        ...options,
        url: '/user_permission_override',
        method: 'GET',
        params: { page_number, page_size },
    })
}

export async function addUserPermissionOverride(
    payload: import('./type').UserPermOverrideAddReq,
    options: RequestOptions = {},
): Promise<import('@/types/models').UserPermVO> {
    return request<import('@/types/models').UserPermVO>({
        ...options,
        url: '/user_permission_override',
        method: 'POST',
        data: payload,
    })
}

export async function patchUserPermissionOverride(
    payload: import('./type').UserPermOverridePatchReq,
    options: RequestOptions = {},
): Promise<import('@/types/models').UserPermVO> {
    return request<import('@/types/models').UserPermVO>({
        ...options,
        url: '/user_permission_override',
        method: 'PATCH',
        data: payload,
    })
}

export async function deleteUserPermissionOverride(
    payload: import('./type').UserPermOverrideDeleteReq,
    options: RequestOptions = {},
): Promise<string> {
    return request<string>({
        ...options,
        url: '/user_permission_override',
        method: 'DELETE',
        data: payload,
    })
}
