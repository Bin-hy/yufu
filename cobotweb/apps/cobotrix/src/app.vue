<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { logout as authLogout } from './auth'

const route = useRoute()
// On the OAuth callback we render a bare layout (no chrome) so the user
// doesn't briefly see a logged-in shell before being redirected.
const isBareLayout = computed(() => route.name === 'oauth-callback')

function logout() {
  void authLogout()
}
</script>

<template>
  <div v-if="isBareLayout" class="min-h-screen">
    <RouterView />
  </div>
  <div v-else class="min-h-screen flex flex-col">
    <header class="border-b border-border px-6 py-3 flex items-center justify-between">
      <h1 class="text-base font-semibold">cobotrix 管理中心</h1>
      <nav class="flex items-center gap-4 text-sm">
        <RouterLink
          to="/dashboard"
          class="hover:underline"
          active-class="font-medium text-primary"
        >总览</RouterLink>
        <RouterLink
          to="/devices"
          class="hover:underline"
          active-class="font-medium text-primary"
        >设备</RouterLink>
        <RouterLink
          to="/tasks"
          class="hover:underline"
          active-class="font-medium text-primary"
        >任务</RouterLink>
        <button class="text-muted-foreground hover:text-foreground" @click="logout">
          退出
        </button>
      </nav>
    </header>
    <main class="flex-1 p-6">
      <RouterView />
    </main>
  </div>
</template>
