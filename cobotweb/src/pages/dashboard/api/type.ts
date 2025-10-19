/**
 * 返回当前登陆的用户信息
 */
export type UserVOData = ApiResponseData<{
    id: string
    nickname: string | null
    role : string
    username :string
}>
