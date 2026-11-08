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
        // 登录接口失败时转换为可读的中文错误，避免 axios 拦截器显示英文原始消息
        throw new Error(
            typeof (e as { response?: { data: unknown } }).response?.data === 'string'
                ? String((e as { response: { data: string } }).response.data)
                : '用户名或密码错误',
        )
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
