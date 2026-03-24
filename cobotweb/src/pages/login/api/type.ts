export interface LoginRequestData {
    /** 用户名 */
    username: string
    /** 密码 */
    password: string
}
/** 登陆响应格式 */
export type LoginResponseData = {
    token: string,
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
