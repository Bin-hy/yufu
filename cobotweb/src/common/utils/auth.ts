import type { Router } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'

/**
 * 统一处理退出登录并跳转到登录页。
 * 封装在 util 中，便于各处复用同一逻辑。
 */
export function logoutAndRedirect(router: Router): void {
  const userStore = useUserStore()
  userStore.logout()
  router.push('/login')
}

/**
 * 仅退出登录，不做路由跳转。
 */
export function logoutOnly(): void {
  const userStore = useUserStore()
  userStore.logout()
}
