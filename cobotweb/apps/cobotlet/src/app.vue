<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { RouterLink, RouterView } from 'vue-router'
import { Bot, MonitorSmartphone, ListChecks } from '@lucide/vue'
import { getHello } from './api'

const alias = ref<string | null>(null)
onMounted(async () => {
  try {
    const info = await getHello()
    alias.value = info.alias || null
  } catch {
    // alias stays null — display falls back to title only
  }
})

const navItems = [
  { to: '/devices', label: '设备', icon: MonitorSmartphone },
  { to: '/dispatch', label: '任务派发', icon: ListChecks },
]
</script>

<template>
  <div class="min-h-screen flex flex-col bg-background">
    <!-- top header -->
    <header class="h-14 border-b border-border flex items-center gap-3 px-4 shrink-0">
      <Bot class="w-5 h-5 text-primary shrink-0" />
      <span class="text-sm font-semibold">cobotlet</span>
      <span v-if="alias" class="text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded font-mono">
        {{ alias }}
      </span>
    </header>

    <!-- body: sidebar + content -->
    <div class="flex flex-1 min-h-0">
      <!-- sidebar -->
      <aside class="w-44 shrink-0 border-r border-border py-3 px-2">
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
