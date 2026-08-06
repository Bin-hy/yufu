import { createRouter, createWebHistory, type RouteRecordRaw, type Router } from 'vue-router'
import { isAuthenticated, redirectToLogin } from './auth'

const routes: RouteRecordRaw[] = [
  {
    path: '/oauth/callback',
    name: 'oauth-callback',
    component: () => import('./pages/callback.vue'),
    meta: { requiresAuth: false },
  },
  {
    path: '/logout',
    name: 'logout',
    component: () => import('./pages/logout.vue'),
    meta: { requiresAuth: false },
  },
  {
    path: '/',
    redirect: '/dashboard',
  },
  {
    path: '/dashboard',
    name: 'dashboard',
    component: () => import('./pages/dashboard.vue'),
    meta: { requiresAuth: true },
  },
  {
    path: '/devices',
    name: 'devices',
    component: () => import('./pages/devices.vue'),
    meta: { requiresAuth: true },
  },
  {
    path: '/tasks',
    name: 'tasks',
    component: () => import('./pages/tasks.vue'),
    meta: { requiresAuth: true },
  },
  {
    path: '/mcp',
    component: () => import('./pages/mcp/index.vue'),
    meta: { requiresAuth: true },
    redirect: '/mcp/overview',
    children: [
      { path: 'overview', name: 'mcp-overview', component: () => import('./pages/mcp/overview.vue') },
      { path: 'tools', name: 'mcp-tools', component: () => import('./pages/mcp/tools.vue') },
      { path: 'sessions', name: 'mcp-sessions', component: () => import('./pages/mcp/sessions.vue') },
      { path: 'audit', name: 'mcp-audit', component: () => import('./pages/mcp/audit.vue') },
      { path: 'connect', name: 'mcp-connect', component: () => import('./pages/mcp/connect.vue') },
      { path: 'tokens', name: 'mcp-tokens', component: () => import('./pages/mcp/tokens.vue') },
    ],
  },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})

export function registerGuard(router: Router) {
  router.beforeEach(async (to, _from, next) => {
    const requiresAuth = to.meta.requiresAuth !== false
    if (requiresAuth && !isAuthenticated()) {
      // Full-page redirect to cobotsrv — no `next()` call, navigation aborts.
      await redirectToLogin(to.fullPath)
      return
    }
    next()
  })
}
