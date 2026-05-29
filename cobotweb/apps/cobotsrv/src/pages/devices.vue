<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { RefreshCw, Search, ChevronRight, ChevronDown, CircleDot, Circle } from '@lucide/vue'
import { listDevices } from '../api'
import type { DeviceVO } from '@cobotweb/shared'

const devices = ref<DeviceVO[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const selectedCobotlet = ref<string>('')
const searchQuery = ref('')
const expandedKeys = ref(new Set<string>())
const lastRefreshed = ref<Date | null>(null)
let autoRefreshTimer: ReturnType<typeof setInterval> | null = null

onMounted(async () => {
  await refresh()
  autoRefreshTimer = setInterval(refresh, 15_000)
})

onUnmounted(() => {
  if (autoRefreshTimer) clearInterval(autoRefreshTimer)
})

async function refresh() {
  loading.value = true
  error.value = null
  try {
    devices.value = await listDevices()
    lastRefreshed.value = new Date()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function toggleExpand(key: string) {
  if (expandedKeys.value.has(key)) {
    expandedKeys.value.delete(key)
  } else {
    expandedKeys.value.add(key)
  }
  expandedKeys.value = new Set(expandedKeys.value) // trigger reactivity
}

function rowKey(d: DeviceVO) {
  return `${d.cobotlet_alias}/${d.device_id}`
}

const cobotletAliases = computed(() =>
  [...new Set(devices.value.map((d) => d.cobotlet_alias))].sort(),
)

const filteredDevices = computed(() => {
  let list = devices.value
  if (selectedCobotlet.value) {
    list = list.filter((d) => d.cobotlet_alias === selectedCobotlet.value)
  }
  const q = searchQuery.value.trim().toLowerCase()
  if (q) {
    list = list.filter(
      (d) =>
        d.device_id.toLowerCase().includes(q) ||
        d.cobotlet_alias.toLowerCase().includes(q) ||
        d.capabilities?.some((c) => c.name.toLowerCase().includes(q)),
    )
  }
  return list
})

function formatRelativeTime(ts: string | null): string {
  if (!ts) return '—'
  const diff = Date.now() - new Date(ts).getTime()
  if (diff < 0) return '刚刚'
  if (diff < 60_000) return `${Math.floor(diff / 1000)}s`
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h`
  return `${Math.floor(diff / 86_400_000)}d`
}

function formatTimestamp(ts: string | null): string {
  if (!ts) return '—'
  return new Date(ts).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}
</script>

<template>
  <div class="space-y-4">
    <!-- toolbar -->
    <div class="flex flex-wrap items-center gap-2">
      <h2 class="text-lg font-semibold mr-2">设备</h2>

      <!-- cobotlet filter -->
      <select
        v-model="selectedCobotlet"
        class="rounded-md border border-border bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
      >
        <option value="">全部 cobotlet</option>
        <option v-for="alias in cobotletAliases" :key="alias" :value="alias">{{ alias }}</option>
      </select>

      <!-- search -->
      <div class="relative flex-1 min-w-40">
        <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground pointer-events-none" />
        <input
          v-model="searchQuery"
          placeholder="搜索设备ID / 能力…"
          class="w-full pl-8 pr-3 py-1.5 rounded-md border border-border bg-transparent text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <!-- refresh -->
      <button
        class="inline-flex items-center gap-1.5 rounded-md border border-border bg-transparent px-3 py-1.5 text-sm hover:bg-muted transition-colors"
        :class="{ 'opacity-50 cursor-not-allowed': loading }"
        :disabled="loading"
        @click="refresh"
      >
        <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': loading }" />
        刷新
      </button>

      <span v-if="lastRefreshed" class="text-xs text-muted-foreground hidden sm:block">
        已刷新 {{ formatRelativeTime(lastRefreshed.toISOString()) }}前
      </span>
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <!-- table -->
    <div class="rounded-lg border border-border bg-card overflow-hidden">
      <table class="min-w-full text-sm">
        <thead>
          <tr class="bg-muted text-left text-xs text-muted-foreground">
            <th class="w-6 px-2 py-2.5"></th>
            <th class="px-3 py-2.5">Cobotlet</th>
            <th class="px-3 py-2.5">设备 ID</th>
            <th class="px-3 py-2.5">能力</th>
            <th class="px-3 py-2.5">状态</th>
            <th class="px-3 py-2.5">心跳</th>
            <th class="px-3 py-2.5 hidden md:table-cell">注册时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading && devices.length === 0">
            <td class="px-3 py-6 text-muted-foreground text-center" colspan="7">加载中…</td>
          </tr>
          <tr v-else-if="filteredDevices.length === 0">
            <td class="px-3 py-6 text-muted-foreground text-center" colspan="7">
              未找到设备。请确保 cobotlet 已连接 MQTT。
            </td>
          </tr>

          <template v-for="d in filteredDevices" :key="rowKey(d)">
            <!-- main row -->
            <tr
              class="border-t border-border hover:bg-muted/40 cursor-pointer select-none"
              @click="toggleExpand(rowKey(d))"
            >
              <td class="px-2 py-2.5 text-muted-foreground">
                <component
                  :is="expandedKeys.has(rowKey(d)) ? ChevronDown : ChevronRight"
                  class="w-3.5 h-3.5"
                />
              </td>
              <td class="px-3 py-2.5 font-mono text-xs">{{ d.cobotlet_alias }}</td>
              <td class="px-3 py-2.5 font-mono text-xs">{{ d.device_id }}</td>
              <td class="px-3 py-2.5">
                <span
                  v-for="c in d.capabilities"
                  :key="c.name"
                  class="mr-1 mb-0.5 inline-block rounded bg-muted px-1.5 py-0.5 text-xs"
                >{{ c.name }}</span>
              </td>
              <td class="px-3 py-2.5">
                <span class="inline-flex items-center gap-1 text-xs" :class="d.online ? 'text-green-600' : 'text-muted-foreground'">
                  <component :is="d.online ? CircleDot : Circle" class="w-3 h-3" />
                  {{ d.online ? '在线' : '离线' }}
                </span>
              </td>
              <td class="px-3 py-2.5 text-xs text-muted-foreground">
                {{ formatRelativeTime(d.last_heartbeat) }}
              </td>
              <td class="px-3 py-2.5 text-xs text-muted-foreground hidden md:table-cell">
                {{ formatTimestamp(d.registered_at) }}
              </td>
            </tr>

            <!-- expanded capability detail -->
            <tr v-if="expandedKeys.has(rowKey(d))" class="border-t border-border bg-muted/20">
              <td colspan="7" class="px-6 py-3">
                <div class="space-y-1.5">
                  <div class="text-xs font-medium text-muted-foreground mb-2">能力详情</div>
                  <div
                    v-for="c in d.capabilities"
                    :key="c.name"
                    class="flex items-start gap-3 text-xs"
                  >
                    <span class="font-medium w-32 shrink-0">{{ c.name }}</span>
                    <span class="font-mono text-muted-foreground">{{ c.method }} {{ c.path }}</span>
                    <span v-if="c.tags?.length" class="text-muted-foreground">
                      [{{ c.tags.join(', ') }}]
                    </span>
                  </div>
                  <div v-if="!d.capabilities?.length" class="text-xs text-muted-foreground">
                    无能力信息
                  </div>
                </div>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>

    <div class="text-xs text-muted-foreground">
      显示 {{ filteredDevices.length }} / {{ devices.length }} 台设备，每 15 秒自动刷新
    </div>
  </div>
</template>
