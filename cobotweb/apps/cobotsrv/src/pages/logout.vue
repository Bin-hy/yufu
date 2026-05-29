<script setup lang="ts">
import { LogOut } from '@lucide/vue'
import { redirectToLoginForm } from '../auth'

async function login() {
  await redirectToLoginForm()
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-background">
    <div class="flex flex-col items-center gap-6 text-center max-w-sm">
      <div class="w-16 h-16 rounded-full bg-muted flex items-center justify-center">
        <LogOut class="w-8 h-8 text-muted-foreground" />
      </div>
      <div>
        <h1 class="text-xl font-semibold mb-1">已退出登录</h1>
        <p class="text-sm text-muted-foreground">你的会话已安全结束。</p>
      </div>
      <button
        class="inline-flex items-center gap-2 rounded-md bg-primary px-6 py-2.5 text-sm font-medium text-primary-foreground hover:opacity-90 transition-opacity"
        @click="login"
      >
        重新登录
      </button>
    </div>
  </div>
</template>
