import { request } from "@/http/axios"
import type { PageResult } from "@/types/pagination"
import type { AuthorityVO } from "@/types/models"
import type { AuthRoleAddBatchReq, AuthRoleDeleteReq } from './type'

export const getAuthoritiesByPage = async (
  page_number: number,
  page_size: number
): Promise<PageResult<AuthorityVO>> => {
  return request<PageResult<AuthorityVO>>({
    url: '/authority',
    method: 'GET',
    params: { page_number, page_size }
  })
}

// 查询指定 roleId 的所有权限
export async function getAuthorityByRoleId(roleId: string): Promise<PageResult<AuthorityVO>> {
  const MAX_INT = 8000
  return request<PageResult<AuthorityVO>>({
    url: `/role_authority/${roleId}`,
    method: 'GET',
    params: { page_number: 1, page_size: MAX_INT }
  })
}

// 批量给角色插入权限
export async function addRoleAuthBatch(
  batchInsertReq: AuthRoleAddBatchReq
): Promise<PageResult<AuthorityVO[]>> {
  return request<PageResult<AuthorityVO[]>>({
    url: `/role_authority/batch`,
    method: 'post',
    data: batchInsertReq
  })
}

// 批量删除角色权限
export async function deleteRoleAuthBatch(batchDeleteReq: AuthRoleDeleteReq) {
  return request<string>({
    url: `/role_authority/batch`,
    method: 'delete',
    data: batchDeleteReq
  })
}