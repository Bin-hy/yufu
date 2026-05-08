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
