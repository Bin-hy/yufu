import { request } from "@/http/axios"
import { emitToast } from "@/utils/toast"
import type { PageResult } from "@/types/pagination"
import type { RoleVO } from "@/types/models"
import type {
  Authority,
  AuthorityInsertReq,
  AuthorityUpdateReq,
  Role,
  AuthorityAssignReq
} from './type'

// 后端 AuthorityVO（snake_case）
interface AuthorityVOResp {
  id: string
  name: string
  authority: string
  create_time?: string
  update_time?: string
}

function toAuthority(vo: AuthorityVOResp): Authority {
  return {
    id: String(vo.id),
    name: String(vo.name),
    authority: String(vo.authority),
    createTime: vo.create_time ?? '',
    updateTime: vo.update_time ?? ''
  }
}

function isAuthorityVOResp(x: any): x is AuthorityVOResp {
  return x && typeof x.id === 'string' && typeof x.name === 'string' && typeof x.authority === 'string'
}

// 获取权限（默认取第一页，大页容量），并转换字段命名
export const getAllAuthorities = async (): Promise<Authority[]> => {
  try {
    const res = await request<{ data: PageResult<AuthorityVOResp> }>({
      url: '/authority',
      method: 'GET',
      params: { page_number: 1, page_size: 100 }
    })
    const page = res?.data
    const list = Array.isArray(page?.data_list) ? page!.data_list : []
    return list.filter(isAuthorityVOResp).map(toAuthority)
  } catch (error) {
    emitToast({ type: 'error', message: '获取权限列表失败' })
    return []
  }
}

// 添加权限
export const addAuthority = async (req: AuthorityInsertReq): Promise<Authority> => {
  const res = await request<{ data: AuthorityVOResp }>({
    url: '/authority',
    method: 'POST',
    data: req
  })
  const vo = res?.data
  if (!vo || !isAuthorityVOResp(vo)) {
    throw new Error('返回数据格式不正确')
  }
  return toAuthority(vo)
}

// 创建权限（与 addAuthority 相同）
export const createAuthority = addAuthority

// 更新权限（PATCH /authority，携带 id）
export const updateAuthority = async (_id: string, req: AuthorityUpdateReq): Promise<Authority> => {
  const res = await request<{ data: AuthorityVOResp }>({
    url: '/authority',
    method: 'PATCH',
    data: req
  })
  const vo = res?.data
  if (!vo || !isAuthorityVOResp(vo)) {
    throw new Error('返回数据格式不正确')
  }
  return toAuthority(vo)
}

// 删除权限（DELETE /authority，body: { id }）
export const deleteAuthority = async (id: string): Promise<boolean> => {
  try {
    const res = await request<{ data: string }>({
      url: '/authority',
      method: 'DELETE',
      data: { id }
    })
    const msg = res?.data
    return typeof msg === 'string' ? true : true
  } catch (error) {
    emitToast({ type: 'error', message: '删除权限失败' })
    return false
  }
}

// 获取所有角色（分页接口，取第一页大页容量）
export const getAllRoles = async (): Promise<Role[]> => {
  try {
    const res = await request<{ data: PageResult<RoleVO> }>({
      url: '/role',
      method: 'GET',
      params: { page_number: 1, page_size: 100 }
    })
    const page = res?.data
    return Array.isArray(page?.data_list) ? page!.data_list : []
  } catch (error) {
    emitToast({ type: 'error', message: '获取角色列表失败' })
    return []
  }
}

// 获取角色权限（后端暂未提供端点）
export const getRoleAuthorities = async (_roleId: string): Promise<Authority[]> => {
  emitToast({ type: 'warning', message: '后端暂未提供获取角色权限的接口' })
  return []
}

// 分配权限给角色（后端暂未提供端点）
export const assignAuthorities = async (_req: AuthorityAssignReq): Promise<boolean> => {
  emitToast({ type: 'warning', message: '后端暂未提供角色权限分配接口' })
  return false
}

export const getAuthoritiesByPage = async (page_number: number, page_size: number): Promise<PageResult<Authority>> => {
  try {
    const res = await request<{ data: PageResult<AuthorityVOResp> }>({
      url: '/authority',
      method: 'GET',
      params: { page_number, page_size }
    })
    const page = res?.data
    const list = Array.isArray(page?.data_list) ? page!.data_list : []
    return {
      page_number: page?.page_number ?? page_number,
      page_size: page?.page_size ?? page_size,
      total_pages: page?.total_pages ?? 1,
      data_list: list.filter(isAuthorityVOResp).map(toAuthority)
    }
  } catch (error) {
    emitToast({ type: 'error', message: '获取权限分页失败' })
    return { page_number, page_size, total_pages: 1, data_list: [] }
  }
}
