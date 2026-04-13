<script lang="ts" setup>
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import { setRememberUser, removeRememberUser, getRememberUser } from '@/common/cache/localstorage'
import { loginApi } from './api'
import UiInput from '@/components/ui/input/input.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import UiButton from '@/components/ui/button/button.vue'

const form = reactive({
  username: '',
  password: '',
  remember: false
})

const errors = reactive({
  username: '',
  password: ''
})

const loginError = ref('')
const loading = ref(false)
const router = useRouter()
const userStore = useUserStore()

function validateForm(): boolean {
  let isValid = true
  errors.username = ''
  errors.password = ''
  if (!form.username.trim()) {
    errors.username = '请输入用户名'
    isValid = false
  }
  if (!form.password) {
    errors.password = '请输入密码'
    isValid = false
  }
  return isValid
}

async function handleLogin() {
  loginError.value = ''
  if (!validateForm()) return
  try {
    loading.value = true
    const data = await loginApi({ username: form.username, password: form.password })
    userStore.setUserInfo({ username: data.username, token: data.access_token, nickname: data.nickname })
    if (form.remember) {
      setRememberUser(form.username)
    } else {
      removeRememberUser()
    }
    router.push('/dashboard')
  } catch (error) {
    loginError.value = error instanceof Error ? (error.message || '用户名或密码错误') : '用户名或密码错误'
  } finally {
    loading.value = false
  }
}

const rememberedUser = getRememberUser()
if (rememberedUser) {
  form.username = rememberedUser
  form.remember = true
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-muted p-4">
    <div class="w-full max-w-md bg-card text-card-foreground rounded-2xl shadow-xl overflow-hidden">
      <!-- 头部 -->
      <div class="bg-gradient-to-br from-primary to-secondary p-8 text-white">
        <div class="flex flex-col items-center gap-3">
          <div class="w-14 h-14 rounded-2xl bg-white/20 flex items-center justify-center">
            <svg class="w-7 h-7 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
            </svg>
          </div>
          <div class="text-center">
            <h1 class="text-2xl font-bold tracking-tight">账号登录</h1>
            <p class="text-white/70 text-sm mt-1">欢迎回来，请登录您的账号</p>
          </div>
        </div>
      </div>

      <div class="p-8 space-y-5">
        <!-- 行内错误提示 -->
        <transition
          enter-active-class="transition-all duration-200 ease-out"
          enter-from-class="opacity-0 -translate-y-1"
          leave-active-class="transition-all duration-150 ease-in"
          leave-to-class="opacity-0 -translate-y-1"
        >
          <div v-if="loginError" class="flex items-start gap-3 rounded-xl border border-destructive/30 bg-destructive/8 px-4 py-3">
            <svg class="w-4 h-4 text-destructive mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z" />
            </svg>
            <span class="text-sm text-destructive">{{ loginError }}</span>
          </div>
        </transition>

        <form class="space-y-5" @submit.prevent="handleLogin">
          <!-- 用户名 -->
          <div>
            <label for="username" class="block mb-1.5 text-sm font-medium text-foreground">用户名</label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                <svg class="h-4 w-4 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                </svg>
              </div>
              <UiInput
                id="username"
                v-model="form.username"
                type="text"
                :class="'pl-10' + (errors.username ? ' border-destructive' : '')"
                placeholder="请输入用户名"
                @blur="validateForm"
              />
            </div>
            <p v-if="errors.username" class="text-destructive text-xs mt-1">{{ errors.username }}</p>
          </div>

          <!-- 密码 -->
          <div>
            <label for="password" class="block mb-1.5 text-sm font-medium text-foreground">密码</label>
            <div class="relative">
              <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
                <svg class="h-4 w-4 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                </svg>
              </div>
              <UiInput
                id="password"
                v-model="form.password"
                type="password"
                :class="'pl-10' + (errors.password ? ' border-destructive' : '')"
                placeholder="请输入密码"
                @blur="validateForm"
              />
            </div>
            <p v-if="errors.password" class="text-destructive text-xs mt-1">{{ errors.password }}</p>
          </div>

          <!-- 记住我 + 忘记密码 -->
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <UiCheckbox id="remember" v-model="form.remember" />
              <label for="remember" class="text-sm text-muted-foreground cursor-pointer select-none">记住我</label>
            </div>
            <a href="#" class="text-sm text-primary hover:text-primary/80 transition-colors">忘记密码？</a>
          </div>

          <UiButton type="submit" size="lg" class="w-full" :disabled="loading">
            <span v-if="loading" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
            登录
          </UiButton>
        </form>

        <!-- 注册 + 提示 -->
        <div class="flex items-center justify-between pt-1">
          <p class="text-sm text-muted-foreground">
            还没有账号？
            <router-link to="/register" class="font-medium text-primary hover:text-primary/80 transition-colors">立即注册</router-link>
          </p>

          <!-- hover 提示 -->
          <div class="relative group">
            <button type="button" class="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors rounded px-1 py-0.5">
              <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10" />
                <path d="M12 16v-4" />
                <path d="M12 8h.01" />
              </svg>
              登录提示
            </button>
            <div class="pointer-events-none absolute right-0 bottom-full mb-2 w-64 rounded-xl border border-border bg-popover text-popover-foreground shadow-lg opacity-0 translate-y-1 group-hover:opacity-100 group-hover:translate-y-0 transition-all duration-150 p-3 z-50 text-sm">
              默认超级管理账户与密码均为
              <code class="mx-1 rounded-md bg-muted px-1.5 py-0.5 text-foreground font-mono text-xs">sadmin</code>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
