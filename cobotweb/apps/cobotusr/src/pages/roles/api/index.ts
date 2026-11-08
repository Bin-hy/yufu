import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type { RoleVO } from '@/types/models'
import type { RolesPage, RoleAddReq, RolePatchReq, AuthRoleAddReq, AuthRoleDeleteReq } from './type'

export async function getRolesByPage(
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
): Promise<RolesPage> {
    return await request<RolesPage>({
        ...options,
        url: '/role',
        method: 'GET',
        params: { page_number, page_size },
    })
}

export async function addRole(payload: RoleAddReq, options: RequestOptions = {}): Promise<RoleVO> {
    return await request<RoleVO>({
        ...options,
        url: '/role',
        method: 'POST',
        data: payload,
    })
}

export async function patchRole(
    payload: RolePatchReq,
    options: RequestOptions = {},
): Promise<RoleVO> {
    return await request<RoleVO>({
        ...options,
        url: '/role',
        method: 'PATCH',
        data: payload,
    })
}

export async function addRoleAuthority(
    payload: AuthRoleAddReq,
    options: RequestOptions = {},
): Promise<boolean> {
    try {
        await request<unknown>({
            ...options,
            url: '/role_authority',
            method: 'POST',
            data: payload,
        })
        return true
    } catch {
        // Axios 拦截器已统一弹错；409 视作已存在不算失败
        return false
    }
}

export async function deleteRoleAuthority(
    payload: AuthRoleDeleteReq,
    options: RequestOptions = {},
): Promise<boolean> {
    try {
        await request<{ data: string }>({
            ...options,
            url: '/role_authority',
            method: 'DELETE',
            data: payload,
        })
        return true
    } catch {
        return false
    }
}
