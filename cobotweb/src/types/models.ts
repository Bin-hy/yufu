export interface RoleVO {
  id: string
  role_enum: string
  name: string
  create_time: string
  update_time: string
}

export interface UserVO {
  id: string
  username: string
  nickname?: string | null
  role?: RoleVO | null
  role_id: string
  create_time?: string
  update_time?: string
}

export interface AuthorityVO{
    authority_uri: string
    create_time: string
    id: string
    name: string
    update_time: string
}

export interface RecordingVO{
    create_time: string
    id: string
    name: string
    user_id: string
    user_vo: UserVO
}
