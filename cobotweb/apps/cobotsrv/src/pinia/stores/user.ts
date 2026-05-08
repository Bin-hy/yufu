import { defineStore } from 'pinia'
import { ref } from 'vue'
import { getToken, setToken, removeToken } from "@/common/cache/cookies.ts";

// 用户信息接口
export interface UserInfo {
  username: string
  token: string
  nickname: string
  // 可以根据需要添加其他用户信息字段
}

// 创建用户 store
export const useUserStore = defineStore('user', () => {
  // 用户信息状态
  const userInfo = ref<UserInfo | null>(null)
  const isLoggedIn = ref(false)

  // 设置用户信息
  const setUserInfo = (info: UserInfo) => {
    userInfo.value = info
    isLoggedIn.value = true

    // token 仅存入 cookie，不放 localStorage（降低 XSS 风险）
    setToken(info.token)
    const { token: _token, ...safeInfo } = info
    localStorage.setItem('userInfo', JSON.stringify(safeInfo))
  }

  // 清除用户信息
  const clearUserInfo = () => {
    userInfo.value = null
    isLoggedIn.value = false

    // 从本地存储清除用户信息
    removeToken()
    localStorage.removeItem('userInfo')
  }

  // 退出登录（清除用户信息的别名方法）
  const logout = () => {
    clearUserInfo()
  }

  // 初始化用户信息（从本地存储恢复）
  const initUserInfo = () => {
    const storedUserInfo = localStorage.getItem('userInfo')
    const token = getToken()
    if (storedUserInfo && token) {
      try {
        const info = JSON.parse(storedUserInfo)
        userInfo.value = { ...info, token }
        isLoggedIn.value = true
      } catch (error) {
        console.error('Failed to parse stored user info:', error)
        localStorage.removeItem('userInfo')
      }
    }
  }

  return {
    userInfo,
    isLoggedIn,
    setUserInfo,
    clearUserInfo,
    logout,
    initUserInfo
  }
})