import { request } from '@/http/axios'

export interface UserInfo {
  id: number
  username: string
  nickname: string | null
  role_id: number
  create_time: string
  update_time: string
}

export function getCurrentUser() {
  return request<UserInfo>({ url: 'user', method: 'get' })
}

export function patchMyProfile(data: { user_id: number; nickname?: string; password?: string }) {
  return request<void>({ url: 'user', method: 'patch', data })
}
