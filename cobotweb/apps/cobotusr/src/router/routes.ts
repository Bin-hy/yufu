import type { RouteRecordRaw } from 'vue-router'
import Layout from '@/layouts/index.vue'
const queryKeys = ['page_number', 'page_size', 'q', 'roleId', 'application_id', 'user_id']
const meta = (title: string, extra = {}) => ({
    title,
    workspace: true,
    requiresAuth: true,
    persistQueryKeys: queryKeys,
    ...extra,
})
export const workspaceRoutes: RouteRecordRaw[] = [
    {
        path: '/dashboard',
        name: 'dashboard-home',
        component: () => import('@/pages/dashboard/views/dashboard-home.vue'),
        meta: meta('工作台', { affix: true }),
    },
    {
        path: '/users',
        name: 'user-list',
        component: () => import('@/pages/users/views/user-list.vue'),
        meta: meta('用户管理'),
    },
    {
        path: '/users/create',
        name: 'create-user',
        component: () => import('@/pages/users/views/user-create.vue'),
        meta: meta('创建用户'),
    },
    {
        path: '/roles',
        name: 'role-list',
        component: () => import('@/pages/roles/views/role-list.vue'),
        meta: meta('角色管理'),
    },
    {
        path: '/authority',
        name: 'authority-list',
        component: () => import('@/pages/authority/views/authority-list.vue'),
        meta: meta('API 权限'),
    },
    {
        path: '/authority/assign',
        name: 'authority-assign',
        component: () => import('@/pages/authority/views/authority-assign.vue'),
        meta: meta('API 权限分配'),
    },
    {
        path: '/permissions',
        name: 'permission-list',
        component: () => import('@/pages/permissions/views/permission-list.vue'),
        meta: meta('资源权限'),
    },
    {
        path: '/permissions/assign',
        name: 'permission-assign',
        component: () => import('@/pages/permissions/views/permission-assign.vue'),
        meta: meta('资源权限分配'),
    },
    {
        path: '/applications',
        name: 'application-list',
        component: () => import('@/pages/applications/views/application-list.vue'),
        meta: meta('应用注册'),
    },
    {
        path: '/authorized-apps',
        name: 'authorized-apps-list',
        component: () => import('@/pages/authorized-apps/views/authorized-apps-list.vue'),
        meta: meta('已授权应用'),
    },
    {
        path: '/recordings',
        name: 'recordings-list',
        component: () => import('@/pages/recordings/views/recordings-list.vue'),
        meta: meta('操作记录'),
    },
    {
        path: '/profile',
        name: 'profile',
        component: () => import('@/pages/profile/index.vue'),
        meta: meta('个人中心', { hidden: true }),
    },
]
const oldNames: Record<string, string> = {
    users: '/users',
    roles: '/roles',
    authority: '/authority',
    permissions: '/permissions',
    applications: '/applications',
    'authorized-apps': '/authorized-apps',
    recordings: '/recordings',
}
export const constantRoutes: RouteRecordRaw[] = [
    {
        path: '/redirect',
        component: () => import('@/pages/redirect/index.vue'),
        meta: { requiresAuth: false, hidden: true },
    },
    {
        path: '/redirect/:path(.*)',
        component: () => import('@/pages/redirect/index.vue'),
        meta: { requiresAuth: false, hidden: true },
    },
    {
        path: '/403',
        name: '403',
        component: () => import('@/pages/errors/403.vue'),
        meta: { requiresAuth: false, hidden: true, title: '无法访问' },
    },
    {
        path: '/404',
        name: '404',
        component: () => import('@/pages/errors/404.vue'),
        meta: { requiresAuth: false, hidden: true, title: '页面不存在' },
    },
    {
        path: '/login',
        name: 'login',
        component: () => import('@/pages/login/index.vue'),
        meta: { requiresAuth: false, title: '登录' },
    },
    {
        path: '/register',
        name: 'register',
        component: () => import('@/pages/login/register.vue'),
        meta: { requiresAuth: false, title: '注册' },
    },
    {
        path: '/oidc/login',
        name: 'oidc-login',
        component: () => import('@/pages/oidc-login/index.vue'),
        meta: { requiresAuth: false, title: '登录并授权' },
    },
    {
        path: '/oidc/consent',
        name: 'oidc-consent',
        component: () => import('@/pages/oidc-consent/index.vue'),
        meta: { requiresAuth: true, title: '应用授权' },
    },
    {
        path: '/',
        component: Layout,
        redirect: '/dashboard',
        meta: { requiresAuth: true },
        children: workspaceRoutes,
    },
    { path: '/dashboard/introduction', name: 'dashboard-introduction', redirect: '/dashboard' },
    { path: '/oauth', redirect: '/applications' },
    { path: '/oauth/:pathMatch(.*)*', redirect: '/applications' },
    ...Object.entries(oldNames).map(([name, redirect]) => ({
        path: '/legacy/' + name,
        name,
        redirect,
    })),
    {
        path: '/:pathMatch(.*)*',
        name: 'not-found',
        redirect: '/404',
        meta: { requiresAuth: false },
    },
]
