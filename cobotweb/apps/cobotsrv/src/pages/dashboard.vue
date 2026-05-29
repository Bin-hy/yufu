<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  LayoutGrid,
  Wifi,
  WifiOff,
  Zap,
  CircleDot,
  Circle,
  Send,
} from '@lucide/vue'
import { listDevices, submitTaskSync, type SubmitTaskPayload } from '../api'
import type { DeviceVO, TaskResult } from '@cobotweb/shared'

const devices = ref<DeviceVO[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

onMounted(async () => {
  loading.value = true
  try {
    devices.value = await listDevices()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
})

const stats = computed(() => {
  const total = devices.value.length
  const online = devices.value.filter((d) => d.online).length
  const cobotlets = new Set(devices.value.map((d) => d.cobotlet_alias)).size
  const totalCapabilities = devices.value.reduce(
    (sum, d) => sum + (d.capabilities?.length ?? 0),
    0,
  )
  return { total, online, offline: total - online, cobotlets, totalCapabilities }
})

// Top-5 devices sorted by last_heartbeat desc
const recentDevices = computed(() =>
  [...devices.value]
    .sort((a, b) => {
      const ta = a.last_heartbeat ? new Date(a.last_heartbeat).getTime() : 0
      const tb = b.last_heartbeat ? new Date(b.last_heartbeat).getTime() : 0
      return tb - ta
    })
    .slice(0, 5),
)

function formatRelativeTime(ts: string | null): string {
  if (!ts) return '—'
  const diff = Date.now() - new Date(ts).getTime()
  if (diff < 0) return '刚刚'
  if (diff < 60_000) return `${Math.floor(diff / 1000)}秒前`
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}分钟前`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}小时前`
  return `${Math.floor(diff / 86_400_000)}天前`
}

// Quick task form
const onlineDevices = computed(() => devices.value.filter((d) => d.online))
const cobotletAliases = computed(() =>
  [...new Set(onlineDevices.value.map((d) => d.cobotlet_alias))].sort(),
)
const quickCobotlet = ref('')
const quickCapability = ref('')
const quickSubmitting = ref(false)
const quickResult = ref<TaskResult | null>(null)
const quickError = ref<string | null>(null)

const devicesForQuickCobotlet = computed(() =>
  onlineDevices.value.filter((d) => d.cobotlet_alias === quickCobotlet.value),
)
const capabilitiesForQuick = computed(() => {
  const caps = new Map<string, string>()
  devicesForQuickCobotlet.value.forEach((d) =>
    d.capabilities?.forEach((c) => caps.set(c.name, d.device_id)),
  )
  return [...caps.entries()]
})

function onQuickCobotletChange() {
  quickCapability.value = capabilitiesForQuick.value[0]?.[0] ?? ''
  quickResult.value = null
  quickError.value = null
}

async function quickSubmit() {
  if (!quickCobotlet.value || !quickCapability.value) return
  quickSubmitting.value = true
  quickResult.value = null
  quickError.value = null
  try {
    const payload: SubmitTaskPayload = {
      cobotlet_alias: quickCobotlet.value,
      capability_name: quickCapability.value,
    }
    quickResult.value = await submitTaskSync(payload)
  } catch (e: unknown) {
    quickError.value = e instanceof Error ? e.message : String(e)
  } finally {
    quickSubmitting.value = false
  }
}

const statCards = computed(() => [
  { label: 'Cobotlet 数', value: stats.value.cobotlets, icon: LayoutGrid, color: 'text-foreground' },
  { label: '在线设备', value: stats.value.online, icon: Wifi, color: 'text-green-600' },
  { label: '离线设备', value: stats.value.offline, icon: WifiOff, color: 'text-muted-foreground' },
  { label: '能力总数', value: stats.value.totalCapabilities, icon: Zap, color: 'text-amber-500' },
])
</script>

<template>
  <div class="space-y-6">
    <h2 class="text-lg font-semibold">总览</h2>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <!-- stat cards -->
    <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
      <div
        v-for="card in statCards"
        :key="card.label"
        class="rounded-lg border border-border bg-card p-4 flex items-start gap-3"
      >
        <div class="mt-0.5 p-2 rounded-md bg-muted">
          <component :is="card.icon" class="w-4 h-4" :class="card.color" />
        </div>
        <div>
          <div class="text-xs text-muted-foreground mb-0.5">{{ card.label }}</div>
          <div class="text-2xl font-bold leading-none" :class="card.color">
            <span v-if="loading" class="text-muted-foreground text-base">—</span>
            <span v-else>{{ card.value }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- lower panels -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <!-- recent active devices -->
      <div class="rounded-lg border border-border bg-card p-4">
        <h3 class="text-sm font-medium mb-3">最近活跃设备</h3>
        <div v-if="loading" class="text-sm text-muted-foreground">加载中…</div>
        <div v-else-if="recentDevices.length === 0" class="text-sm text-muted-foreground">
          暂无设备
        </div>
        <ul v-else class="divide-y divide-border">
          <li
            v-for="d in recentDevices"
            :key="`${d.cobotlet_alias}/${d.device_id}`"
            class="flex items-center justify-between py-2 gap-2"
          >
            <div class="flex items-center gap-2 min-w-0">
              <component
                :is="d.online ? CircleDot : Circle"
                class="w-3.5 h-3.5 shrink-0"
                :class="d.online ? 'text-green-500' : 'text-muted-foreground'"
              />
              <span class="font-mono text-xs truncate">{{ d.cobotlet_alias }}/{{ d.device_id }}</span>
            </div>
            <span class="text-xs text-muted-foreground shrink-0">
              {{ formatRelativeTime(d.last_heartbeat) }}
            </span>
          </li>
        </ul>
      </div>

      <!-- quick task -->
      <div class="rounded-lg border border-border bg-card p-4">
        <h3 class="text-sm font-medium mb-3">快速任务</h3>
        <div class="space-y-3">
          <div>
            <label class="block text-xs text-muted-foreground mb-1">Cobotlet</label>
            <select
              v-model="quickCobotlet"
              class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
              @change="onQuickCobotletChange"
            >
              <option value="">— 选择 cobotlet —</option>
              <option v-for="alias in cobotletAliases" :key="alias" :value="alias">{{ alias }}</option>
            </select>
          </div>
          <div>
            <label class="block text-xs text-muted-foreground mb-1">能力</label>
            <select
              v-model="quickCapability"
              class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
              :disabled="!quickCobotlet"
            >
              <option value="">— 选择能力 —</option>
              <option v-for="[name] in capabilitiesForQuick" :key="name" :value="name">{{ name }}</option>
            </select>
          </div>
          <button
            :disabled="quickSubmitting || !quickCobotlet || !quickCapability"
            class="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity"
            @click="quickSubmit"
          >
            <Send class="w-3.5 h-3.5" />
            {{ quickSubmitting ? '派发中…' : '派发任务' }}
          </button>

          <div v-if="quickError" class="rounded bg-destructive/10 text-destructive text-xs p-2">
            {{ quickError }}
          </div>
          <pre v-else-if="quickResult" class="rounded bg-muted text-xs p-2 overflow-auto max-h-28 whitespace-pre-wrap">{{ JSON.stringify(quickResult, null, 2) }}</pre>
        </div>
      </div>
    </div>
  </div>
</template>
