<script lang="ts" setup>
import { ref, reactive, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { loginApi } from '@/pages/login/api'
import { useUserStore } from '@/pinia/stores/user'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import { showModal } from '@/utils/modal'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()
const loading = ref(false)
const appName = ref('')

const form = reactive({
  username: '',
  password: '',
})

const errors = reactive({
  username: '',
  password: '',
})

// Read the client_id from query to show which app is requesting
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
  if (!validateForm()) return

  try {
    loading.value = true
    const data = await loginApi({
      username: form.username,
      password: form.password,
    })

    userStore.setUserInfo({
      username: data.username,
      token: data.access_token,
      nickname: data.nickname,
    })

    // After login, go directly to the consent page (same frontend)
    // No need to round-trip through backend /oidc/authorize again
    router.push({ name: 'oidc-consent', query: route.query })
  } catch (error) {
    console.error('登录失败:', error)
    if (error instanceof Error) {
      showModal({
        title: '登录失败',
        content: error.message || '请检查用户名和密码',
      })
    }
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-muted p-4">
    <div class="w-full max-w-md bg-card text-card-foreground rounded-lg shadow-xl overflow-hidden">
      <!-- Header -->
      <div class="bg-gradient-to-r from-primary to-secondary p-8 text-white">
        <div class="text-center mb-2">
          <div class="w-16 h-16 rounded-full bg-white bg-opacity-20 flex items-center justify-center mx-auto">
            <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"></path>
            </svg>
          </div>
        </div>
        <h1 class="text-2xl font-bold text-center">Sign in to authorize</h1>
        <p class="text-white text-opacity-80 text-center mt-1">
          Sign in to continue to the application
        </p>
      </div>

      <div class="p-8">
        <form @submit.prevent="handleLogin">
          <!-- Username -->
          <div class="mb-6">
            <label for="username" class="block mb-2 text-sm font-medium text-foreground">
              Username
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
              />
            </div>
            <p v-if="errors.username" class="text-destructive text-sm mt-1">{{ errors.username }}</p>
          </div>

          <!-- Password -->
          <div class="mb-6">
            <label for="password" class="block mb-2 text-sm font-medium text-foreground">
              Password
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
              />
            </div>
            <p v-if="errors.password" class="text-destructive text-sm mt-1">{{ errors.password }}</p>
          </div>

          <UiButton
            type="submit"
            size="lg"
            class="w-full"
            :disabled="loading"
          >
            <span v-if="loading" class="mr-2 h-5 w-5 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
            Sign in
          </UiButton>
        </form>
      </div>
    </div>
  </div>
</template>
