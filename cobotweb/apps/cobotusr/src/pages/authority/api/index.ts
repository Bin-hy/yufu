import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type { PageResult } from '@/types/pagination'
import type { AuthorityVO } from '@/types/models'
import type { AuthRoleAddBatchReq, AuthRoleDeleteReq, AuthorityInsertReq } from './type'

export const getAuthoritiesByPage = async (
    page_number: number,
    page_size: number,
    options: RequestOptions = {},
): Promise<PageResult<AuthorityVO>> => {
    return request<PageResult<AuthorityVO>>({
        ...options,
        url: '/authority',
        method: 'GET',
        params: { page_number, page_size },
    })
}

// 查询指定 roleId 的所有权限
export async function getAuthorityByRoleId(
    roleId: number,
    page_number = 1,
    page_size = 100,
    options: RequestOptions = {},
): Promise<PageResult<AuthorityVO>> {
    return request<PageResult<AuthorityVO>>({
        ...options,
        url: `/role_authority/${roleId}`,
        method: 'GET',
        params: { page_number, page_size },
    })
}

// 批量给角色插入权限
export async function addRoleAuthBatch(
    batchInsertReq: AuthRoleAddBatchReq,
    options: RequestOptions = {},
): Promise<PageResult<AuthorityVO[]>> {
    return request<PageResult<AuthorityVO[]>>({
        ...options,
        url: `/role_authority/batch`,
        method: 'post',
        data: batchInsertReq,
    })
}

// 批量删除角色权限
export async function deleteRoleAuthBatch(
    batchDeleteReq: AuthRoleDeleteReq,
    options: RequestOptions = {},
) {
    return request<string>({
        ...options,
        url: `/role_authority/batch`,
        method: 'delete',
        data: batchDeleteReq,
    })
}

// 新增权限
export async function createAuthority(
    payload: AuthorityInsertReq,
    options: RequestOptions = {},
): Promise<AuthorityVO> {
    return request<AuthorityVO>({
        ...options,
        url: '/authority',
        method: 'POST',
        data: payload,
    })
}
