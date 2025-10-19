<script lang="ts" setup>
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import { setRememberUser, removeRememberUser, getRememberUser } from '@/common/cache/localstorage'
import { loginApi } from './api'
import { showModal } from '@/utils/modal'
// 表单数据
const form = reactive({
  username: '',
  password: '',
  remember: false
})

// 表单验证错误信息
const errors = reactive({
  username: '',
  password: ''
})

// 登录状态
const loading = ref(false)
const router = useRouter()
const userStore = useUserStore()

// 表单验证
function validateForm (): boolean{
    let isValid = true

    // 重置错误信息
    errors.username = ''
    errors.password = ''

    // 验证用户名
    if (!form.username.trim()) {
    errors.username = '请输入用户名'
    isValid = false
    }

    // 验证密码
    if (!form.password) {
    errors.password = '请输入密码'
    isValid = false
    } else if (form.password.length < 6) {
    errors.password = '密码长度不能少于6位'
    isValid = false
    }

    return isValid
}

// 登录处理
async function handleLogin () {
    // 验证表单
    if (!validateForm()) {
        return
    }
    try {
        loading.value = true
        // 测试错误处理
        let data = await loginApi({
            // role: "admin",
            username: form.username,
            password: form.password
        })

        // 假设登录成功，存储用户信息
        userStore.setUserInfo({
            username: data.data.username,
            token: data.data.token,
            nickname: data.data.nickname
        })

        // 如果勾选了记住我，可以在这里存储到本地存储
        if (form.remember) {
            setRememberUser(form.username)
        } else {
            removeRememberUser()
        }

        // 登录成功后跳转到首页或其他页面
        router.push('/dashboard')

    } catch (error) {
        console.error('登录失败:', error)
        showModal({
            title: '登录失败',
            content: error.message || '请检查用户名和密码'
        })
    } finally {
        loading.value = false
    }
}

// 检查是否有记住的用户
const rememberedUser = localStorage.getItem('rememberedUser')
if (rememberedUser) {
  form.username = rememberedUser
  form.remember = true
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-base-200 p-4">
    <div class="w-full max-w-md bg-white rounded-lg shadow-xl overflow-hidden">
      <!-- 登录表单头部 - 使用daisyui的卡片 -->
      <div class="bg-gradient-to-r from-primary to-secondary p-8 text-white">
        <div class="text-center mb-2">
          <div class="w-16 h-16 rounded-full bg-white bg-opacity-20 flex items-center justify-center mx-auto">
            <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
            </svg>
          </div>
        </div>
        <h1 class="text-2xl font-bold text-center">账号登录</h1>
        <p class="text-white text-opacity-80 text-center mt-1">欢迎回来，请登录您的账号</p>
      </div>

      <!-- 登录表单内容 - 使用daisyui的表单组件 -->
      <div class="p-8">
        <form @submit.prevent="handleLogin">
          <!-- 用户名输入框 - 使用daisyui的输入框 -->
          <div class="mb-6">
            <label for="username" class="label block mb-2 text-base font-medium text-gray-700">
              用户名
            </label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                <svg class="h-5 w-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
                </svg>
              </div>
              <input
                id="username"
                v-model="form.username"
                type="text"
                class="input input-lg w-full pl-10 pr-4 py-3 border-2 rounded-lg focus:ring-2 focus:ring-primary/50 focus:border-primary transition duration-200 ease-in-out"
                :class="{ 'border-error': errors.username, 'border-gray-300': !errors.username }"
                placeholder="请输入用户名"
                @blur="validateForm"
              />
            </div>
            <p v-if="errors.username" class="text-error text-sm mt-1">{{ errors.username }}</p>
          </div>

          <!-- 密码输入框 - 使用daisyui的输入框 -->
          <div class="mb-6">
            <label for="password" class="label block mb-2 text-base font-medium text-gray-700">
              密码
            </label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                <svg class="h-5 w-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"></path>
                </svg>
              </div>
              <input
                id="password"
                v-model="form.password"
                type="password"
                class="input input-lg w-full pl-10 pr-4 py-3 border-2 rounded-lg focus:ring-2 focus:ring-primary/50 focus:border-primary transition duration-200 ease-in-out"
                :class="{ 'border-error': errors.password, 'border-gray-300': !errors.password }"
                placeholder="请输入密码"
                @blur="validateForm"
              />
            </div>
            <p v-if="errors.password" class="text-error text-sm mt-1">{{ errors.password }}</p>
          </div>

          <!-- 记住我和忘记密码 - 使用daisyui的复选框 -->
          <div class="flex items-center justify-between mb-6">
            <div class="flex items-center space-x-2">
              <input
                id="remember"
                v-model="form.remember"
                type="checkbox"
                class="checkbox h-5 w-5 rounded border-primary/30 focus:ring-primary focus:ring-offset-0"
              />
              <label for="remember" class="text-sm text-gray-700 cursor-pointer hover:text-primary transition-colors">
                记住我
              </label>
            </div>
            <a href="#" class="text-sm font-medium text-primary hover:text-primary/80 transition-colors">
              忘记密码？
            </a>
          </div>

          <!-- 登录按钮 - 使用daisyui的按钮 -->
          <button
            type="submit"
            class="btn btn-lg btn-primary w-full py-3 font-medium rounded-lg bg-primary text-white hover:bg-primary/90 focus:ring-4 focus:ring-primary/30 transition-all duration-200 active:scale-95"
            :class="{ 'opacity-70 cursor-not-allowed': loading }"
            :disabled="loading"
          >
            <span v-if="loading" class="mr-2 h-5 w-5 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
            登录
          </button>
        </form>

        <!-- 注册账号提示 -->
        <div class="mt-8 text-center">
          <p class="text-sm text-gray-600">
            还没有账号？
            <a href="#" class="font-medium text-primary hover:text-primary/80 transition-colors">
              立即注册
            </a>
          </p>
        </div>
      </div>
    </div>

  </div>
</template>
