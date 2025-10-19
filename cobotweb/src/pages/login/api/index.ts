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
