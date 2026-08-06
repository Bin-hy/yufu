<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { LayoutDashboard, MonitorSmartphone, ListChecks, Plug, LogOut, Bot } from '@lucide/vue'
import { logout as authLogout, fetchUserInfo, type UserInfo } from './auth'

const route = useRoute()
const isBareLayout = computed(() =>
  route.name === 'oauth-callback' || route.name === 'logout',
)

const userInfo = ref<UserInfo | null>(null)
onMounted(async () => {
  userInfo.value = await fetchUserInfo()
})

const displayName = computed(
  () =>
    userInfo.value?.nickname ||
    userInfo.value?.name ||
    userInfo.value?.preferred_username ||
    `UID ${userInfo.value?.sub ?? '?'}`,
)

const navItems = [
  { to: '/dashboard', label: '总览', icon: LayoutDashboard },
  { to: '/devices', label: '设备', icon: MonitorSmartphone },
  { to: '/tasks', label: '任务', icon: ListChecks },
  { to: '/mcp', label: 'MCP', icon: Plug },
]

function logout() {
  void authLogout()
}
</script>

<template>
  <!-- bare layout: callback + logout pages -->
  <RouterView v-if="isBareLayout" />

  <!-- main admin layout -->
  <div v-else class="min-h-screen flex flex-col bg-background">
    <!-- top header -->
    <header class="h-14 border-b border-border flex items-center justify-between px-4 shrink-0">
      <div class="flex items-center gap-2.5 text-sm font-semibold">
        <Bot class="w-5 h-5 text-primary" />
        CoBotrix 管理中心
      </div>
      <div class="flex items-center gap-3">
        <span v-if="userInfo" class="text-xs text-muted-foreground hidden sm:block">
          {{ displayName }}
        </span>
        <button
          class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
          @click="logout"
        >
          <LogOut class="w-3.5 h-3.5" />
          退出
        </button>
      </div>
    </header>

    <!-- body: sidebar + content -->
    <div class="flex flex-1 min-h-0">
      <!-- sidebar -->
      <aside class="w-52 shrink-0 border-r border-border flex flex-col py-3 px-2">
        <nav class="flex flex-col gap-0.5">
          <RouterLink
            v-for="item in navItems"
            :key="item.to"
            :to="item.to"
            class="flex items-center gap-3 rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
            active-class="bg-muted text-foreground font-medium"
          >
            <component :is="item.icon" class="w-4 h-4 shrink-0" />
            {{ item.label }}
          </RouterLink>
        </nav>
      </aside>

      <!-- main content -->
      <main class="flex-1 overflow-auto p-6">
        <RouterView />
      </main>
    </div>
  </div>
</template>
