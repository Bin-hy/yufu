<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { handleCallback, redirectToLogin } from '../auth'

const route = useRoute()
const router = useRouter()
const error = ref<string>('')

onMounted(async () => {
  const code = route.query.code as string | undefined
  const state = route.query.state as string | undefined
  const errParam = route.query.error as string | undefined

  if (errParam) {
    error.value = `授权被拒绝: ${errParam}${route.query.error_description ? ` — ${route.query.error_description}` : ''}`
    return
  }
  if (!code || !state) {
    error.value = '缺少 code 或 state 参数'
    return
  }

  try {
    const dest = await handleCallback(code, state)
    router.replace(dest)
  } catch (e) {
    error.value = e instanceof Error ? e.message : '登录失败'
  }
})

async function retry() {
  await redirectToLogin()
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center p-6">
    <div v-if="!error" class="text-muted-foreground text-sm">登录中…</div>
    <div v-else class="max-w-md w-full rounded-lg border border-destructive/30 bg-destructive/5 p-6">
      <h1 class="text-base font-semibold mb-2">登录失败</h1>
      <p class="text-sm text-destructive break-words mb-4">{{ error }}</p>
      <button
        class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90"
        @click="retry"
      >
        重新登录
      </button>
    </div>
  </div>
</template>
