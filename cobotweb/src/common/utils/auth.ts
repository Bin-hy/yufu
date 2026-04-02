import type { Router } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import { logoutRequest } from '@/pages/login/api'

/**
 * 统一处理退出登录并跳转到登录页。
 * 封装在 util 中，便于各处复用同一逻辑。
 */
export async function logoutAndRedirect(router: Router): Promise<void> {
  const userStore = useUserStore()
  try {
    await logoutRequest()
  } catch (e) {
    // 即使请求失败也执行本地退出与跳转
  } finally {
    userStore.logout()
    router.push('/login')
  }
}

/**
 * 仅退出登录，不做路由跳转。
 */
export async function logoutOnly(): Promise<void> {
  const userStore = useUserStore()
  try { await logoutRequest() } catch { /* ignore */ }
  userStore.logout()
}
