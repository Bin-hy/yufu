import { request } from "@/http/axios"
import type * as Auth from "./type"

/** 登录并返回 Token */
export function loginApi(data: Auth.LoginRequestData) {
  return request<Auth.LoginResponseData>({
    url: "login", // auth_old/login
    method: "post",
    data
  })
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
