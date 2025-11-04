import { createRouter } from 'vue-router'
import { routerConfig } from './config'
import { flatMultiLevelRoutes } from './helper'
import { RegistryRouterGuard } from './guard'
import { constantRoutes } from './routes'

export const router = createRouter({
    history: routerConfig.history,
    routes: routerConfig.thirdLevelRouteCache ? flatMultiLevelRoutes(constantRoutes) : constantRoutes
})

// 添加导航守卫
RegistryRouterGuard(router)
