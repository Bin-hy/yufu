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
    { path: '/account', name: 'account-profile', component: () => import('@/pages/account/views/profile.vue'), meta: meta('账户中心') },
    { path: '/account/apps', name: 'account-apps', component: () => import('@/pages/account/views/apps.vue'), meta: meta('可访问应用') },
    { path: '/account/consents', name: 'account-consents', component: () => import('@/pages/account/views/consents.vue'), meta: meta('资料授权') },
    { path: '/account/sessions', name: 'account-sessions', component: () => import('@/pages/account/views/sessions.vue'), meta: meta('登录会话') },
    { path: '/developer/apps', name: 'developer-apps', component: () => import('@/pages/developer/views/apps.vue'), meta: meta('开发者控制台', { qualification: 'developer' }) },
    { path: '/developer/apps/:id', name: 'developer-app', component: () => import('@/pages/developer/views/app-detail.vue'), meta: meta('应用接入', { qualification: 'developer', tabIdentity: 'params', tabParamKeys: ['id'] }) },
    { path: '/admin/accounts', name: 'admin-accounts', component: () => import('@/pages/admin/views/accounts.vue'), meta: meta('企业账户', { qualification: 'administrator' }) },
    { path: '/admin/apps', name: 'admin-apps', component: () => import('@/pages/developer/views/apps.vue'), meta: meta('企业应用', { qualification: 'administrator' }) },
    { path: '/admin/apps/:id', name: 'admin-app', component: () => import('@/pages/developer/views/app-detail.vue'), meta: meta('企业应用配置', { qualification: 'administrator', tabIdentity: 'params', tabParamKeys: ['id'] }) },
    { path: '/admin/groups', name: 'admin-groups', component: () => import('@/pages/admin/views/groups.vue'), meta: meta('用户组', { qualification: 'administrator' }) },
    { path: '/admin/audit', name: 'admin-audit', component: () => import('@/pages/admin/views/audit.vue'), meta: meta('身份审计', { qualification: 'administrator' }) },
    { path: '/admin/deliveries', name: 'admin-deliveries', component: () => import('@/pages/admin/views/deliveries.vue'), meta: meta('退出通知', { qualification: 'administrator' }) },
    { path: '/admin/migrations', name: 'admin-migrations', component: () => import('@/pages/admin/views/migrations.vue'), meta: meta('历史接入纳管', { qualification: 'administrator' }) },
    {
        path: '/dashboard',
        name: 'dashboard-home',
        component: () => import('@/pages/dashboard/views/dashboard-home.vue'),
        meta: meta('工作台', { affix: true }),
    },
    // 旧 RBAC 管理面已从产品中退役：保留 URL 兼容，直接落到新控制台的对应页面。
    { path: '/users', name: 'user-list', redirect: '/admin/accounts', meta: meta('用户管理', { qualification: 'administrator', hidden: true }) },
    { path: '/users/create', name: 'create-user', redirect: '/admin/accounts', meta: meta('企业邀请', { qualification: 'administrator', hidden: true }) },
    { path: '/roles', name: 'role-list', redirect: '/admin/groups', meta: meta('角色管理', { qualification: 'administrator', hidden: true }) },
    { path: '/authority', name: 'authority-list', redirect: '/admin/apps', meta: meta('API 权限', { qualification: 'administrator', hidden: true }) },
    { path: '/authority/assign', name: 'authority-assign', redirect: '/admin/apps', meta: meta('API 权限分配', { qualification: 'administrator', hidden: true }) },
    { path: '/permissions', name: 'permission-list', redirect: '/admin/apps', meta: meta('资源权限', { qualification: 'administrator', hidden: true }) },
    { path: '/permissions/assign', name: 'permission-assign', redirect: '/admin/apps', meta: meta('资源权限分配', { qualification: 'administrator', hidden: true }) },
    { path: '/applications', name: 'application-list', redirect: '/developer/apps', meta: meta('应用注册', { qualification: 'developer', hidden: true }) },
    { path: '/authorized-apps', name: 'authorized-apps-list', redirect: '/account/consents', meta: meta('已授权应用') },
    { path: '/recordings', name: 'recordings-list', redirect: '/admin/audit', meta: meta('操作记录', { qualification: 'administrator', hidden: true }) },
    { path: '/profile', name: 'profile', redirect: '/account', meta: meta('个人中心', { hidden: true }) },
]
const oldNames: Record<string, string> = {
    users: '/admin/accounts',
    roles: '/admin/groups',
    authority: '/admin/apps',
    permissions: '/admin/apps',
    applications: '/developer/apps',
    'authorized-apps': '/account/consents',
    recordings: '/admin/audit',
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
    { path: '/account/action', name: 'account-action', component: () => import('@/pages/account/views/action.vue'), meta: { requiresAuth: false, title: '企业账户邀请与重置' } },
    {
        path: '/oidc/login',
        name: 'oidc-login',
        component: () => import('@/pages/oidc-consent/index.vue'),
        meta: { requiresAuth: false, title: '登录并授权' },
    },
    {
        path: '/oidc/consent',
        name: 'oidc-consent',
        component: () => import('@/pages/oidc-consent/index.vue'),
        meta: { requiresAuth: false, title: '应用授权' },
    },
    {
        path: '/',
        component: Layout,
        redirect: '/account',
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
