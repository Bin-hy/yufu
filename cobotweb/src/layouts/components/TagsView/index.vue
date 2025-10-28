<template>
  <div class="bg-muted border-b border-border px-2 py-1 flex items-center gap-1">
    <div v-for="tab in tabs.visited" :key="tab.path" class="flex items-center">
      <div
        class="rounded-lg px-3 py-1 mr-1 cursor-pointer flex items-center gap-2 border border-transparent"
        :class="{ 'bg-card text-card-foreground border-border': tab.path === tabs.activePath, 'hover:bg-accent hover:text-accent-foreground': tab.path !== tabs.activePath }"
        @click="go(tab.path)"
      >
        <span>{{ tab.title }}</span>
        <UiButton
          v-if="!tab.affix"
          variant="ghost"
          size="sm"
          @click.stop="close(tab.path)"
        >✕</UiButton>
      </div>
    </div>


  </div>
</template>

<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTabsStore } from '@/pinia/stores/tabs'
import UiButton from '@/components/ui/button/button.vue'

const router = useRouter()
const route = useRoute()
const tabs = useTabsStore()

onMounted(() => {
  tabs.initHome('/dashboard', String(route.meta?.dashboardTitle ?? '仪表盘首页'))
  tabs.add(route)
})

watch(() => route.fullPath, () => {
  tabs.add(route)
})

function go(path: string) {
  if (path !== route.path) router.push(path)
}
function close(path: string) {
  tabs.remove(path)
}
function closeOthers() {
  tabs.closeOthers(route.path)
}
function refresh() {
  router.replace({ path: route.path, query: { ...route.query, _r: Date.now() } })
}
</script>
