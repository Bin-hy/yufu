import type { Router} from 'vue-router'
import { getToken } from "@/common/cache/cookies"

export function RegistryRouterGuard(router: Router) {
    // 统一认证守卫：仅在需要认证的路由上校验 Token
    router.beforeEach((to, from, next) => {
        const requiresAuth = to.meta.requiresAuth !== false
        if (requiresAuth && !getToken()) {
            next({ name: 'login', replace: true })
            return
        }
        next()
    })

    // 后置路由守卫（可用于埋点、标题设置等）
    router.afterEach((to, from) => {
        // 达到路由后执行的操作
    })
}
