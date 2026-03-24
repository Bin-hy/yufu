<script lang="ts" setup>
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import { setRememberUser, removeRememberUser, getRememberUser } from '@/common/cache/localstorage'
import { loginApi } from './api'
import { showModal } from '@/utils/modal'
import UiInput from '@/components/ui/input/input.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import UiButton from '@/components/ui/button/button.vue'
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
            username: data.username,
            token: data.token,
            nickname: data.nickname
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
        if (error instanceof Error){
            showModal({
                title: '登录失败',
                content: error.message || '请检查用户名和密码'
            })
        }

    } finally {
        loading.value = false
    }
}

// 检查是否有记住的用户（使用统一工具函数，避免键名不一致）
const rememberedUser = getRememberUser()
if (rememberedUser) {
  form.username = rememberedUser
  form.remember = true
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-muted p-4">
    <div class="w-full max-w-md bg-card text-card-foreground rounded-lg shadow-xl overflow-hidden">
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

      <div class="p-8">

        <form @submit.prevent="handleLogin">
          <div class="mb-6">
            <label for="username" class="block mb-2 text-sm font-medium text-foreground">
              用户名
            </label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                <svg class="h-5 w-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
                </svg>
              </div>
              <UiInput
                id="username"
                v-model="form.username"
                type="text"
                :class="errors.username ? 'pl-10 border-destructive' : 'pl-10'"
                placeholder="请输入用户名"
                @blur="validateForm"
              />
            </div>
            <p v-if="errors.username" class="text-destructive text-sm mt-1">{{ errors.username }}</p>
          </div>

          <!-- 密码输入框 - 使用daisyui的输入框 -->
          <div class="mb-6">
            <label for="password" class="block mb-2 text-sm font-medium text-foreground">
              密码
            </label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                <svg class="h-5 w-5 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"></path>
                </svg>
              </div>
              <UiInput
                id="password"
                v-model="form.password"
                type="password"
                :class="errors.password ? 'pl-10 border-destructive' : 'pl-10'"
                placeholder="请输入密码"
                @blur="validateForm"
              />
            </div>
            <p v-if="errors.password" class="text-destructive text-sm mt-1">{{ errors.password }}</p>
          </div>

          <div class="flex items-center justify-between mb-6">
            <div class="flex items-center space-x-2">
              <UiCheckbox
                id="remember"
                v-model="form.remember"
              />
              <label for="remember" class="text-sm text-foreground cursor-pointer hover:text-primary transition-colors">
                记住我
              </label>
            </div>
            <a href="#" class="text-sm font-medium text-primary hover:text-primary/80 transition-colors">
              忘记密码？
            </a>
          </div>

          <UiButton
            type="submit"
            size="lg"
            class="w-full"
            :disabled="loading"
          >
            <span v-if="loading" class="mr-2 h-5 w-5 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
            登录
          </UiButton>
        </form>

        <!-- 注册账号提示 -->
        <div class="mt-8 text-center">
          <p class="text-sm text-gray-600">
            还没有账号？
            <router-link to="/register" class="font-medium text-primary hover:text-primary/80 transition-colors">
              立即注册
            </router-link>
          </p>
        </div>

        <!-- 登录提示（悬浮气泡） -->
        <div class="mb-4 flex justify-end">
          <div class="relative group inline-flex items-center">
            <button
              type="button"
              class="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground focus:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded px-1"
              aria-describedby="login-tip"
            >
              <!-- info 图标 -->
              <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="10" />
                <path d="M12 16v-4" />
                <path d="M12 8h.01" />
              </svg>
              <span class="hidden sm:inline">登录提示</span>
            </button>
            <!-- tooltip 内容 -->
            <div
              id="login-tip"
              role="tooltip"
              class="pointer-events-none absolute right-0 top-full mt-2 w-[min(22rem,90vw)] rounded-md border border-border bg-popover text-popover-foreground shadow-lg opacity-0 translate-y-1 group-hover:opacity-100 group-hover:translate-y-0 group-focus-within:opacity-100 group-focus-within:translate-y-0 transition-all duration-150 p-3 z-50"
            >
              <p class="text-sm leading-6">
                <span class="font-medium">提示：</span>
                默认超级管理账户与密码均为
                <code class="mx-1 rounded bg-muted px-1.5 py-0.5 text-foreground">sadmin</code>
              </p>
            </div>
          </div>
        </div>

      </div>
    </div>

  </div>
</template>
