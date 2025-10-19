export interface LoginRequestData {
    // role: string
    /** 用户名 */
    username: string
    /** 密码 */
    password: string
    /** 验证码 */
    // code: string
}
/** 验证码响应格式 */
export type CaptchaResponseData = ApiResponseData<string>

/** 登陆响应格式 */
export type LoginResponseData = ApiResponseData<{
    token: string,
    username: string,
    nickname: string,
 }>
