import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { routerConfig } from './config'
import { flatMultiLevelRoutes } from './helper'
import { useUserStore } from '@/pinia/stores/user'
import { RegistryRouterGuard } from './guard'
/**
 * @name 常驻路由
 * @description 除了 redirect/403/404/login 等隐藏页面，其他页面建议设置唯一的 Name 属性
 */
export const constantRoutes: RouteRecordRaw[] = [
    {
        "path": "/redirect",
        "component": () => import("@/pages/redirect/index.vue"),
        "meta": {requiresAuth: false }  // 不需要认证
    },
    {
        "path": "/403",
        "name": "403",
        "component": () => import("@/pages/errors/403.vue"),
        "meta": {requiresAuth: false }  // 不需要认证
    },
    {
        "path": "/404",
        "name": "404",
        "component": () => import("@/pages/errors/404.vue"),
        "meta": {requiresAuth: false }  // 不需要认证
    },
    {
        "path": "/",
        "name": "login",
        "component": () => import("@/pages/login/index.vue"),
        "meta": {
            requiresAuth: false,
            hidden: true // 隐藏在导航菜单中
        }
    },
    {
        "path": "/dashboard",
        "name": "dashboard",
        "component": () => import("@/pages/dashboard/index.vue"),
        "meta": {
            requiresAuth: true, // 需要登录才能访问
            title: '仪表盘'
        }
    }
]

export const router = createRouter({
    history: routerConfig.history,
    routes: routerConfig.thirdLevelRouteCache ? flatMultiLevelRoutes(constantRoutes) : constantRoutes
})

// 添加导航守卫
RegistryRouterGuard(router)
