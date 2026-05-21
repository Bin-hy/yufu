// 权限添加请求
export interface AuthorityInsertReq {
  name: string;
  authority_uri: string;
  method: string;
}

// 权限更新请求
export interface AuthorityUpdateReq {
  id: number;
  name?: string;
  authority_uri?: string;
  method?: string;
}

// 角色模型（与后端 RoleVO 对齐）
import type { RoleVO } from '@/types/models'
export type Role = RoleVO

// 权限分配请求（批量）
export interface AuthRoleAddBatchReq {
  role_id: number;
  authority_ids: number[];
}

// 分配权限请求
export interface AuthRoleAddReq{
    role_id: number;
    authority_ids: number;
}

// 取消分配权限请求（批量）
export interface AuthRoleDeleteReq{
    role_id: number;
    authority_ids: number[];
}
