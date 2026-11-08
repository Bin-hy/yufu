import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type { PageResult } from '@/types/pagination'
import type { PermissionVO } from '@/types/models'
import type {
    PermissionPage,
    PermissionInsertReq,
    PermissionPatchReq,
    RolePermBatchAddReq,
    RolePermBatchDeleteReq,
} from './type'

// --- Permission CRUD ---

export async function getPermissionsByPage(
    page_number: number,
    page_size: number,
    application_id?: number | null,
    options: RequestOptions = {},
): Promise<PermissionPage> {
    const params: Record<string, string | number> = { page_number, page_size }
    if (application_id != null) params.application_id = application_id
    return request<PermissionPage>({
        ...options,
        url: '/permission',
        method: 'GET',
        params,
    })
}

export async function createPermission(
    payload: PermissionInsertReq,
    options: RequestOptions = {},
): Promise<PermissionVO> {
    return request<PermissionVO>({
        ...options,
        url: '/permission',
        method: 'POST',
        data: payload,
    })
}

export async function patchPermission(
    payload: PermissionPatchReq,
    options: RequestOptions = {},
): Promise<PermissionVO> {
    return request<PermissionVO>({
        ...options,
        url: '/permission',
        method: 'PATCH',
        data: payload,
    })
}

export async function deletePermission(id: number, options: RequestOptions = {}): Promise<string> {
    return request<string>({
        ...options,
        url: '/permission',
        method: 'DELETE',
        data: { id },
    })
}

// --- Role-Permission ---

export async function getRolePermissionsByRoleId(
    roleId: number,
    page_number = 1,
    page_size = 100,
    options: RequestOptions = {},
): Promise<PageResult<PermissionVO>> {
    return request<PageResult<PermissionVO>>({
        ...options,
        url: `/role_permission/${roleId}`,
        method: 'GET',
        params: { page_number, page_size },
    })
}

export async function addRolePermissionBatch(
    payload: RolePermBatchAddReq,
    options: RequestOptions = {},
): Promise<string> {
    return request<string>({
        ...options,
        url: '/role_permission/batch',
        method: 'POST',
        data: payload,
    })
}

export async function deleteRolePermissionBatch(
    payload: RolePermBatchDeleteReq,
    options: RequestOptions = {},
): Promise<string> {
    return request<string>({
        ...options,
        url: '/role_permission/batch',
        method: 'DELETE',
        data: payload,
    })
}
