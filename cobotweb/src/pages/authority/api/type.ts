// 权限添加请求
export interface AuthorityInsertReq {
  name: string;
  authority_uri: string;
}

// 权限更新请求
export interface AuthorityUpdateReq {
  id: string;
  name?: string;
  authority_uri?: string;
}

// 角色模型（与后端 RoleVO 对齐）
import type { RoleVO } from '@/types/models'
export type Role = RoleVO

// 权限分配请求（批量）
export interface AuthRoleAddBatchReq {
  role_id: string;
  authority_ids: string[];
}

// 分配权限请求
export interface AuthRoleAddReq{
    role_id: string;
    authority_ids: string;
}

// 取消分配权限请求（批量）
export interface AuthRoleDeleteReq{
    role_id: string;
    authority_ids: string[];
}
