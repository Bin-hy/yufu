// 权限模型
export interface Authority {
  id: string;
  name: string;
  authority: string; // uri
  createTime: string;
  updateTime: string;
}

// 权限添加请求
export interface AuthorityInsertReq {
  name: string;
  authority: string;
}

// 权限更新请求
export interface AuthorityUpdateReq {
  id: string;
  name?: string;
  authority?: string;
}

// 角色模型（与后端 RoleVO 对齐）
import type { RoleVO } from '@/types/models'
export type Role = RoleVO

// 权限角色关联模型
export interface AuthorityRole {
  id: string;
  roleId?: string;
  authorityId?: string;
  role?: Role;
  authority?: Authority;
}

// 权限分配请求
export interface AuthorityAssignReq {
  roleId: string;
  authorityIds: string[];
}