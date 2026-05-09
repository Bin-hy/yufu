import type { RouteRecordRaw } from 'vue-router'
import Layout from '@/layouts/index.vue'

/**
 * 常驻路由（参考 v3-admin-vite：根路径使用全局布局，重定向到 /dashboard；登录位于 /login）
 */
export const constantRoutes: RouteRecordRaw[] = [
  {
    path: '/redirect',
    component: () => import('@/pages/redirect/index.vue'),
    meta: { requiresAuth: false, hidden: true }
  },
  {
    path: '/403',
    name: '403',
    component: () => import('@/pages/errors/403.vue'),
    meta: { requiresAuth: false ,hidden: true}
  },
  {
    path: '/404',
    name: '404',
    component: () => import('@/pages/errors/404.vue'),
    meta: { requiresAuth: false ,hidden: true}
  },
  {
    path: '/login',
    name: 'login',
    component: () => import('@/pages/login/index.vue'),
    meta: { requiresAuth: false, hidden: true }
  },
  {
    path: '/register',
    name: 'register',
    component: () => import('@/pages/login/register.vue'),
    meta: { requiresAuth: false, hidden: true, title: '用户注册' }
  },
  // OIDC flow pages (standalone, no layout)
  {
    path: '/oidc/login',
    name: 'oidc-login',
    component: () => import('@/pages/oidc-login/index.vue'),
    meta: { requiresAuth: false, hidden: true, title: 'Sign in to authorize' }
  },
  {
    path: '/oidc/consent',
    name: 'oidc-consent',
    component: () => import('@/pages/oidc-consent/index.vue'),
    meta: { requiresAuth: true, hidden: true, title: 'Authorize Application' }
  },
  {
    path: '/',
    component: Layout,
    redirect: '/dashboard',
    meta: { requiresAuth: true, title: '仪表盘' },
    children: [
      // 仪表盘首页
      {
        path: '/dashboard',
        name: 'dashboard-home',
        component: () => import('@/pages/dashboard/views/dashboard-home.vue'),
        meta: { title: '仪表盘首页', affix: true, icon: 'dashboard' },
        children:[
           // 访问 /dashboard 时默认展示系统介绍
           { path: '', redirect: '/dashboard/introduction', meta: { hidden: true } },
            // 仪表盘扩展页（如统计、性能、活动）
            {
                path: 'introduction',
                name: 'dashboard-introduction',
                component: () => import('@/pages/dashboard/components/dashboard-introduction.vue'),
                meta: { title: '系统介绍' }
            },
        ]
      },
      // 用户管理
      {
        path: '/users',
        name: 'users',
        component: () => import('@/pages/users/index.vue'),
        meta: { title: '用户管理', icon: 'people' },
        children: [
          {
            path: '',
            name: 'user-list',
            component: () => import('@/pages/users/views/user-list.vue'),
            meta: { title: '用户列表' }
          },
          {
            path: 'create',
            name: 'create-user',
            component: () => import('@/pages/users/views/user-create.vue'),
            meta: { title: '创建用户' }
          }
        ]
      },
      // 角色管理
      {
        path: '/roles',
        name: 'roles',
        component: () => import('@/pages/roles/index.vue'),
        meta: { title: '角色管理', icon: 'badge' },
        children: [
          {
            path: '',
            name: 'role-list',
            component: () => import('@/pages/roles/views/role-list.vue'),
            meta: { title: '角色列表' }
          }
        ]
      },

      // 权限管理
      {
        path: '/authority',
        name: 'authority',
        component: () => import('@/pages/authority/index.vue'),
        meta: { title: '权限管理', icon: 'security' },
        children: [
          {
            path: '',
            name: 'authority-list',
            component: () => import('@/pages/authority/views/authority-list.vue'),
            meta: { title: '权限列表' }
          },
          {
            path: 'assign',
            name: 'authority-assign',
            component: () => import('@/pages/authority/views/authority-assign.vue'),
            meta: { title: '权限分配' }
          }
        ]
      },
      // 操作记录
      {
        path: '/recordings',
        name: 'recordings',
        component: () => import('@/pages/recordings/index.vue'),
        meta: { title: '操作记录', icon: 'history' },
        children: [
          {
            path: '',
            name: 'recordings-list',
            component: () => import('@/pages/recordings/views/recordings-list.vue'),
            meta: { title: '记录列表' }
          }
        ]
      },
      // 应用管理 (OAuth/OIDC)
      {
        path: '/oauth',
        name: 'oauth',
        component: () => import('@/pages/oauth/index.vue'),
        meta: { title: '应用管理', icon: 'apps' },
        children: [
          {
            path: '',
            name: 'oauth-list',
            component: () => import('@/pages/oauth/views/oauth-list.vue'),
            meta: { title: 'OAuth Apps' }
          },
          {
            path: 'new',
            name: 'oauth-create',
            component: () => import('@/pages/oauth/views/oauth-create.vue'),
            meta: { title: '注册应用' }
          },
          {
            path: ':id',
            name: 'oauth-detail',
            component: () => import('@/pages/oauth/views/oauth-detail.vue'),
            meta: { title: '应用详情' }
          }
        ]
      },
      // Application 注册管理
      {
        path: '/applications',
        name: 'applications',
        component: () => import('@/pages/applications/index.vue'),
        meta: { title: '应用注册', icon: 'apps' },
        children: [
          {
            path: '',
            name: 'application-list',
            component: () => import('@/pages/applications/views/application-list.vue'),
            meta: { title: '应用列表' }
          }
        ]
      },
      // 资源权限管理
      {
        path: '/permissions',
        name: 'permissions',
        component: () => import('@/pages/permissions/index.vue'),
        meta: { title: '资源权限', icon: 'security' },
        children: [
          {
            path: '',
            name: 'permission-list',
            component: () => import('@/pages/permissions/views/permission-list.vue'),
            meta: { title: '权限列表' }
          },
          {
            path: 'assign',
            name: 'permission-assign',
            component: () => import('@/pages/permissions/views/permission-assign.vue'),
            meta: { title: '权限分配' }
          }
        ]
      },
      // 已授权应用
      {
        path: '/authorized-apps',
        name: 'authorized-apps',
        component: () => import('@/pages/authorized-apps/index.vue'),
        meta: { title: '已授权应用', icon: 'key' },
        children: [
          {
            path: '',
            name: 'authorized-apps-list',
            component: () => import('@/pages/authorized-apps/views/authorized-apps-list.vue'),
            meta: { title: '已授权应用' }
          }
        ]
      },
      // 个人中心
      {
        path: '/profile',
        name: 'profile',
        component: () => import('@/pages/profile/index.vue'),
        meta: { title: '个人中心',hidden: true }
      }
    ]
  },
  // 兜底 404 路由（未匹配到任何路由时重定向到 /404）
  { path: '/:pathMatch(.*)*', name: 'not-found', redirect: '/404', meta: { requiresAuth: false, hidden: true } }
]
