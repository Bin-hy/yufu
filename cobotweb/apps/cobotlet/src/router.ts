import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  { path: '/', redirect: '/devices' },
  {
    path: '/devices',
    name: 'devices',
    component: () => import('./pages/devices.vue'),
  },
  {
    path: '/dispatch',
    name: 'dispatch',
    component: () => import('./pages/dispatch.vue'),
  },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})
