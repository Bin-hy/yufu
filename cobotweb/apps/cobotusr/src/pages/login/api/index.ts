import { request } from "@/http/axios"
import type * as Auth from "./type"

/** 登录并返回 Token */
export async function loginApi(data: Auth.LoginRequestData): Promise<Auth.LoginResponseData> {
  try {
    return await request<Auth.LoginResponseData>({
      url: "login",
      method: "post",
      data
    })
  } catch (e: any) {
    // 登录接口失败时转换为可读的中文错误，避免 axios 拦截器显示英文原始消息
    throw new Error(e?.response?.data || '用户名或密码错误')
  }
}

export function registerApi(data: Auth.RegisterRequestData){
    return request<Auth.RegisterResponseData>({
        url: "register", // auth_old/register
        method: "post",
        data
    })
}

// 刷新 access_token
export function refreshTokenRequest() {
  return request<Auth.RefreshRespData>({
    url: "refresh",
    method: "post",
    // withCredentials: true,
  })
}

export function logoutRequest(){
    return request<Auth.LogoutRespData>({
        url: "logout",
        method: "post",
        // withCredentials: true,
    })
}

// 把用户踢下线
export function kickoutUserRequest(data: Auth.KickReqData){
    return request({
        url: "kickout",
        method: "post",
        data,
        // withCredentials: true,
    })
}
