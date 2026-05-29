export interface RoleVO {
  id: number
  name: string
  create_time: string
  update_time: string
}

export interface UserVO {
  id: number
  username: string
  nickname?: string | null
  role?: RoleVO | null
  role_id: number
  create_time?: string
  update_time?: string
}

export interface AuthorityVO{
    authority_uri: string
    create_time: string
    id: number
    name: string
    update_time: string
}

export interface RecordingVO{
    create_time: string
    id: number
    name: string
    authority_uri: string
    method: string
    user_id: number
    user_vo: UserVO
}

export interface UserAuthorityVO{
    allow: boolean,
    user_vo?: UserVO | null,
    authority_vo?: AuthorityVO | null,
    create_time: string,
    update_time: string,
}

export interface ApplicationVO {
    id: number
    app_key: string
    name: string
    description?: string | null
    oidc_client_id?: number | null
    create_time: string
    update_time: string
}

export interface PermissionVO {
    id: number
    application_id: number
    name: string
    description?: string | null
    action: string
    resource_type?: string | null
    resource_pattern?: string | null
    create_time: string
    update_time: string
}

export interface UserPermVO {
    user_id: number
    permission_id: number
    allow: boolean
    create_time: string
    update_time: string
    user_vo?: UserVO | null
    permission_vo?: PermissionVO | null
}

export interface RolePermVO {
    role_id: number
    permission_id: number
    create_time: string
    role?: RoleVO | null
    permission?: PermissionVO | null
}
