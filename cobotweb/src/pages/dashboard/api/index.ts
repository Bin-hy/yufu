import type * as User from "@/pages/dashboard/api/type.ts";
import {request} from "@/http/axios.ts";

/**
 * 获取用户信息
 */
export function userDataApi() {
    return request<User.UserVOData>({
        url: "user", // auth_old/login
        method: "get",
    })
}
