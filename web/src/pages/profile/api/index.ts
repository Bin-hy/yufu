import { request } from '@/http/axios'
import type { RequestOptions } from '@/http/axios'

export interface UserInfo {
    id: number
    username: string
    nickname: string | null
    role_id: number
    create_time: string
    update_time: string
}

export function getCurrentUser(options: RequestOptions = {}) {
    return request<UserInfo>({ url: 'user', method: 'get', ...options })
}

export function patchMyProfile(
    data: { user_id: number; nickname?: string; password?: string },
    options: RequestOptions = {},
) {
    return request<void>({ url: 'user', method: 'patch', data, ...options })
}
