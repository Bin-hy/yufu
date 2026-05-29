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
