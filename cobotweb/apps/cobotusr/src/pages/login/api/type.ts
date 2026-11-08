export interface LoginRequestData {
    /** 用户名 */
    username: string
    /** 密码 */
    password: string
}
/** 登陆响应格式 */
export type LoginResponseData = {
    access_token: string,
    csrf_token: string,
    username: string,
    nickname: string,
 }

 export interface RegisterRequestData {
    /** 用户名 */
    username: string
    /** 密码 */
    password: string
    /** 昵称 */
    nickname: string
 }

export type RegisterResponseData = LoginResponseData

export interface RefreshRespData{
    access_token: string,
    csrf_token: string,
}

export type LogoutRespData = RefreshRespData;

export interface KickReqData{
    sub: number,
}
