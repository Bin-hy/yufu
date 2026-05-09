<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { logout as authLogout, fetchUserInfo, type UserInfo } from './auth'

const route = useRoute()
const isBareLayout = computed(() => route.name === 'oauth-callback')

const userInfo = ref<UserInfo | null>(null)
onMounted(async () => {
  userInfo.value = await fetchUserInfo()
})

const displayName = computed(() =>
  userInfo.value?.nickname || userInfo.value?.name || userInfo.value?.preferred_username || `UID ${userInfo.value?.sub || '?'}`
)

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
      <div class="flex items-center gap-6">
        <nav class="flex items-center gap-4 text-sm">
          <RouterLink to="/dashboard" class="hover:underline" active-class="font-medium text-primary">总览</RouterLink>
          <RouterLink to="/devices" class="hover:underline" active-class="font-medium text-primary">设备</RouterLink>
          <RouterLink to="/tasks" class="hover:underline" active-class="font-medium text-primary">任务</RouterLink>
        </nav>
        <span v-if="userInfo" class="text-xs text-muted-foreground border-l border-border pl-4">
          {{ displayName }}
        </span>
        <button class="text-sm text-muted-foreground hover:text-foreground" @click="logout">退出</button>
      </div>
    </header>
    <main class="flex-1 p-6">
      <RouterView />
    </main>
  </div>
</template>
