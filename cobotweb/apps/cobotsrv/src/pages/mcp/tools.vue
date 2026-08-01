<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Search, RefreshCw, ChevronRight, ChevronDown, CircleDot, Circle } from '@lucide/vue'
import { listTools, type ToolDescriptor } from '../../api'

const tools = ref<ToolDescriptor[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const search = ref('')
const onlyOnline = ref(false)
const expanded = ref<Set<string>>(new Set())

async function load() {
  loading.value = true
  error.value = null
  try {
    tools.value = await listTools()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function toggle(name: string) {
  const next = new Set(expanded.value)
  if (next.has(name)) next.delete(name)
  else next.add(name)
  expanded.value = next
}

function isOnline(t: ToolDescriptor): boolean {
  return t.providers.some((p) => p.online)
}

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase()
  return tools.value.filter((t) => {
    if (onlyOnline.value && !isOnline(t)) return false
    if (!q) return true
    return (
      t.name.toLowerCase().includes(q) ||
      (t.description ?? '').toLowerCase().includes(q)
    )
  })
})

onMounted(load)
</script>

<template>
  <div class="space-y-4">
    <!-- toolbar -->
    <div class="flex items-center gap-2 flex-wrap">
      <div class="relative flex-1 min-w-48">
        <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
        <input
          v-model="search"
          type="text"
          placeholder="搜索工具名 / 描述"
          class="w-full rounded-md border border-border bg-transparent pl-8 pr-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>
      <label class="inline-flex items-center gap-1.5 text-sm text-muted-foreground cursor-pointer select-none">
        <input v-model="onlyOnline" type="checkbox" class="rounded" />
        仅在线
      </label>
      <button
        :disabled="loading"
        class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-2 text-sm hover:bg-muted disabled:opacity-50 transition-colors"
        @click="load"
      >
        <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': loading }" />
        刷新
      </button>
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div class="rounded-lg border border-border bg-card overflow-hidden">
      <div v-if="loading" class="p-4 text-sm text-muted-foreground">加载中…</div>
      <div v-else-if="filtered.length === 0" class="p-4 text-sm text-muted-foreground">暂无工具</div>
      <ul v-else class="divide-y divide-border">
        <li v-for="t in filtered" :key="t.name">
          <button
            class="w-full flex items-center gap-2 px-3 py-2.5 text-left hover:bg-muted/50 transition-colors"
            @click="toggle(t.name)"
          >
            <component :is="expanded.has(t.name) ? ChevronDown : ChevronRight" class="w-4 h-4 shrink-0 text-muted-foreground" />
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <span class="font-mono text-sm font-medium">{{ t.name }}</span>
                <component
                  :is="isOnline(t) ? CircleDot : Circle"
                  class="w-3 h-3 shrink-0"
                  :class="isOnline(t) ? 'text-green-500' : 'text-muted-foreground'"
                />
              </div>
              <div v-if="t.description" class="text-xs text-muted-foreground truncate mt-0.5">{{ t.description }}</div>
            </div>
            <div class="hidden sm:flex items-center gap-1 shrink-0">
              <span
                v-for="p in t.providers"
                :key="`${p.cobotlet_alias}/${p.device_id}`"
                class="inline-flex items-center gap-1 rounded bg-muted px-1.5 py-0.5 text-[11px] font-mono"
                :class="p.online ? 'text-green-600' : 'text-muted-foreground'"
              >
                <component :is="p.online ? CircleDot : Circle" class="w-2.5 h-2.5" />
                {{ p.cobotlet_alias }}
              </span>
            </div>
          </button>

          <!-- expanded: schema -->
          <div v-if="expanded.has(t.name)" class="px-9 pb-3 space-y-2">
            <div class="text-xs text-muted-foreground font-mono">{{ t.method }} {{ t.path }}</div>
            <div>
              <div class="text-xs text-muted-foreground mb-1">inputSchema</div>
              <pre class="rounded-md bg-muted text-xs p-2.5 overflow-auto max-h-72 whitespace-pre-wrap">{{ JSON.stringify(t.input_schema ?? { type: 'object' }, null, 2) }}</pre>
            </div>
          </div>
        </li>
      </ul>
    </div>
  </div>
</template>
