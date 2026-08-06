<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { RefreshCw, Search, ChevronRight, ChevronDown, CircleDot, Circle, Trash2 } from '@lucide/vue'
import { listDevices, deleteDevice, type LocalDevice } from '../api'

const devices = ref<LocalDevice[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const searchQuery = ref('')
const expandedKeys = ref(new Set<string>())
const lastRefreshed = ref<Date | null>(null)
const deletingIds = ref(new Set<string>())
const clearingOffline = ref(false)
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

function toggleExpand(id: string) {
  if (expandedKeys.value.has(id)) {
    expandedKeys.value.delete(id)
  } else {
    expandedKeys.value.add(id)
  }
  expandedKeys.value = new Set(expandedKeys.value)
}

async function removeSingle(id: string, evt: Event) {
  evt.stopPropagation()
  deletingIds.value = new Set(deletingIds.value).add(id)
  try {
    await deleteDevice(id)
    devices.value = devices.value.filter((d) => d.id !== id)
    expandedKeys.value.delete(id)
    expandedKeys.value = new Set(expandedKeys.value)
  } catch (e: unknown) {
    error.value = `删除失败: ${e instanceof Error ? e.message : String(e)}`
  } finally {
    const next = new Set(deletingIds.value)
    next.delete(id)
    deletingIds.value = next
  }
}

const offlineDevices = computed(() => devices.value.filter((d) => d.state === 'offline'))

async function clearOffline() {
  if (offlineDevices.value.length === 0) return
  clearingOffline.value = true
  error.value = null
  const results = await Promise.allSettled(
    offlineDevices.value.map((d) => deleteDevice(d.id)),
  )
  const failed = results.filter((r) => r.status === 'rejected')
  if (failed.length > 0) {
    const msg = (failed[0] as PromiseRejectedResult).reason
    error.value = `部分删除失败: ${msg instanceof Error ? msg.message : String(msg)}`
  }
  // Refresh to get authoritative list regardless of partial failure
  await refresh()
  clearingOffline.value = false
}

const filteredDevices = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return devices.value
  return devices.value.filter(
    (d) =>
      d.id.toLowerCase().includes(q) ||
      d.capabilities?.some((c) => c.name.toLowerCase().includes(q)),
  )
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
</script>

<template>
  <div class="space-y-4">
    <!-- toolbar -->
    <div class="flex flex-wrap items-center gap-2">
      <h2 class="text-lg font-semibold mr-2">设备</h2>

      <div class="relative flex-1 min-w-40">
        <Search class="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground pointer-events-none" />
        <input
          v-model="searchQuery"
          placeholder="搜索设备 ID / 能力…"
          class="w-full pl-8 pr-3 py-1.5 rounded-md border border-border bg-transparent text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <!-- clear offline -->
      <button
        :disabled="clearingOffline || offlineDevices.length === 0"
        class="inline-flex items-center gap-1.5 rounded-md border border-destructive/40 bg-transparent px-3 py-1.5 text-sm text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        :title="`清除 ${offlineDevices.length} 台离线设备`"
        @click="clearOffline"
      >
        <Trash2 class="w-3.5 h-3.5" />
        清除离线 <span v-if="offlineDevices.length > 0">({{ offlineDevices.length }})</span>
      </button>

      <button
        :disabled="loading"
        class="inline-flex items-center gap-1.5 rounded-md border border-border bg-transparent px-3 py-1.5 text-sm hover:bg-muted transition-colors disabled:opacity-50"
        @click="refresh"
      >
        <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': loading }" />
        刷新
      </button>

      <span v-if="lastRefreshed" class="text-xs text-muted-foreground hidden sm:block">
        已刷新 {{ formatRelativeTime(lastRefreshed.toISOString()) }} 前
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
            <th class="px-3 py-2.5">设备 ID</th>
            <th class="px-3 py-2.5">能力</th>
            <th class="px-3 py-2.5">状态</th>
            <th class="px-3 py-2.5">心跳</th>
            <th class="px-2 py-2.5 w-8"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading && devices.length === 0">
            <td class="px-3 py-6 text-center text-muted-foreground" colspan="6">加载中…</td>
          </tr>
          <tr v-else-if="filteredDevices.length === 0">
            <td class="px-3 py-6 text-center text-muted-foreground" colspan="6">
              还没有设备注册到本 cobotlet。
            </td>
          </tr>

          <template v-for="d in filteredDevices" :key="d.id">
            <!-- main row -->
            <tr
              class="border-t border-border hover:bg-muted/40 cursor-pointer select-none"
              @click="toggleExpand(d.id)"
            >
              <td class="px-2 py-2.5 text-muted-foreground">
                <component
                  :is="expandedKeys.has(d.id) ? ChevronDown : ChevronRight"
                  class="w-3.5 h-3.5"
                />
              </td>
              <td class="px-3 py-2.5 font-mono text-xs">{{ d.id }}</td>
              <td class="px-3 py-2.5">
                <span
                  v-for="c in d.capabilities"
                  :key="c.name"
                  class="mr-1 mb-0.5 inline-block rounded bg-muted px-1.5 py-0.5 text-xs"
                >{{ c.name }}</span>
              </td>
              <td class="px-3 py-2.5">
                <span
                  class="inline-flex items-center gap-1 text-xs"
                  :class="d.state === 'ready' || d.state === 'busy' ? 'text-green-600' : 'text-muted-foreground'"
                >
                  <component :is="d.state === 'ready' || d.state === 'busy' ? CircleDot : Circle" class="w-3 h-3" />
                  {{ d.state }}
                </span>
              </td>
              <td class="px-3 py-2.5 text-xs text-muted-foreground">
                {{ formatRelativeTime(d.last_heartbeat) }}
              </td>
              <!-- delete button -->
              <td class="px-2 py-2.5">
                <button
                  :disabled="deletingIds.has(d.id)"
                  class="p-1 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-40"
                  title="删除设备"
                  @click="removeSingle(d.id, $event)"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </td>
            </tr>

            <!-- expanded detail -->
            <tr v-if="expandedKeys.has(d.id)" class="border-t border-border bg-muted/20">
              <td colspan="6" class="px-6 py-3">
                <div class="space-y-2">
                  <div class="flex items-center gap-2 text-xs">
                    <span class="font-medium text-muted-foreground">Service URL:</span>
                    <span class="font-mono text-muted-foreground">{{ d.service_url }}</span>
                  </div>
                  <div class="space-y-1.5">
                    <div class="text-xs font-medium text-muted-foreground">能力详情</div>
                    <div
                      v-for="c in d.capabilities"
                      :key="c.name"
                      class="flex items-start gap-3 text-xs"
                    >
                      <span class="font-medium w-28 shrink-0">{{ c.name }}</span>
                      <span class="font-mono text-muted-foreground">{{ c.method }} {{ c.path }}</span>
                      <span v-if="c.tags?.length" class="text-muted-foreground">
                        [{{ c.tags.join(', ') }}]
                      </span>
                    </div>
                    <div v-if="!d.capabilities?.length" class="text-xs text-muted-foreground">
                      无能力信息
                    </div>
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
