import type { Router} from 'vue-router'
import { getToken } from "@/common/cache/cookies"

export function RegistryRouterGuard(router: Router) {
    // 前置路由守卫
    router.beforeEach((to, from, next) => {
        // 如果需要认证 或者默认未填写 requiresAuth ,// 需要认证
        if(to.meta.requiresAuth == true || to.meta.requiresAuth == undefined) {
            // 检查token
            if(!getToken()){
                // 未登录，重定向到登录页
                router.push({ name: 'login' })
                return false
            }
            // 跳转
            next()
        }else {
            next()
        }
    })

    // 处理 404
    router.beforeEach((to, from, next) => {
        if (to.matched.length === 0) {
                next({ name: '404' })
            } else {
                next()
            }
    })
    // 后置路由守卫
    router.afterEach((to, from) => {
        // 达到路由后执行的操作
    })
}
