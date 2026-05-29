<script lang="ts" setup>
import { ref, reactive, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { loginApi } from '@/pages/login/api'
import { useUserStore } from '@/pinia/stores/user'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()
const loading = ref(false)
const appName = ref('')
const loginError = ref('')

const form = reactive({
  username: '',
  password: '',
})

const errors = reactive({
  username: '',
  password: '',
})

onMounted(() => {
  appName.value = (route.query.client_id as string) || ''
})

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
    router.push({ name: 'oidc-consent', query: route.query })
  } catch (error) {
    loginError.value = error instanceof Error ? (error.message || '用户名或密码错误') : '用户名或密码错误'
  } finally {
    loading.value = false
  }
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
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
            </svg>
          </div>
          <div class="text-center">
            <h1 class="text-2xl font-bold tracking-tight">登录以授权</h1>
            <p class="text-white/70 text-sm mt-1">登录后继续访问应用</p>
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
              />
            </div>
            <p v-if="errors.password" class="text-destructive text-xs mt-1">{{ errors.password }}</p>
          </div>

          <UiButton type="submit" size="lg" class="w-full" :disabled="loading">
            <span v-if="loading" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
            登录
          </UiButton>
        </form>
      </div>
    </div>
  </div>
</template>
