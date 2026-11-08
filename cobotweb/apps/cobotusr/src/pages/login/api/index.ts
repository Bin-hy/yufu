import type { RequestOptions } from '@/http/axios'
import { request } from '@/http/axios'
import type * as Auth from './type'

/** 登录并返回 Token */
export async function loginApi(
    data: Auth.LoginRequestData,
    options: RequestOptions = {},
): Promise<Auth.LoginResponseData> {
    try {
        return await request<Auth.LoginResponseData>({
            ...options,
            url: 'login',
            method: 'post',
            data,
        })
    } catch (e) {
        // 保留 HTTP 层整理后的错误，地址或网络故障也需要可见的诊断信息。
        throw new Error(e instanceof Error ? e.message : '登录失败，请稍后重试')
    }
}

export function registerApi(data: Auth.RegisterRequestData, options: RequestOptions = {}) {
    return request<Auth.RegisterResponseData>({
        ...options,
        url: 'register', // auth_old/register
        method: 'post',
        data,
    })
}

// 刷新 access_token
export function refreshTokenRequest(options: RequestOptions = {}) {
    return request<Auth.RefreshRespData>({
        ...options,
        url: 'refresh',
        method: 'post',
        // withCredentials: true,
    })
}

export function logoutRequest(options: RequestOptions = {}) {
    return request<Auth.LogoutRespData>({
        ...options,
        url: 'logout',
        method: 'post',
        // withCredentials: true,
    })
}

// 把用户踢下线
export function kickoutUserRequest(data: Auth.KickReqData, options: RequestOptions = {}) {
    return request({
        ...options,
        url: 'kickout',
        method: 'post',
        data,
        // withCredentials: true,
    })
}
