export interface RoleVO {
  id: string
  role_enum: string
  name: string
}

export interface UserVO {
  id: string
  username: string
  nickname?: string | null
  role?: RoleVO | null
  create_time?: string
}