<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
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
import type { Capability, DeviceVO, JsonSchemaProperty, TaskResult } from '@cobotweb/shared'

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
  const online = devices.value.filter((d) => d.state === 'ready' || d.state === 'busy').length
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

// Quick task form (only devices that can accept new tasks)
const onlineDevices = computed(() => devices.value.filter((d) => d.state === 'ready'))
const cobotletAliases = computed(() =>
  [...new Set(onlineDevices.value.map((d) => d.cobotlet_alias))].sort(),
)
const quickCobotlet = ref('')
const quickCapability = ref('')
const quickFormData = ref<Record<string, unknown>>({})
const quickSubmitting = ref(false)
const quickResult = ref<TaskResult | null>(null)
const quickError = ref<string | null>(null)

const devicesForQuickCobotlet = computed(() =>
  onlineDevices.value.filter((d) => d.cobotlet_alias === quickCobotlet.value),
)

// Map: capability name → { deviceId, capability }
const capabilitiesForQuick = computed(() => {
  const caps = new Map<string, { deviceId: string; capability: Capability }>()
  devicesForQuickCobotlet.value.forEach((d) =>
    d.capabilities?.forEach((c) =>
      caps.set(c.name, { deviceId: d.device_id, capability: c }),
    ),
  )
  return [...caps.entries()]
})

const selectedQuickEntry = computed(() =>
  capabilitiesForQuick.value.find(([name]) => name === quickCapability.value),
)
const selectedQuickCapability = computed<Capability | undefined>(
  () => selectedQuickEntry.value?.[1].capability,
)
const quickSchema = computed(() => selectedQuickCapability.value?.input_schema ?? null)

watch(selectedQuickCapability, (cap) => {
  if (!cap?.input_schema?.properties) {
    quickFormData.value = {}
    return
  }
  const init: Record<string, unknown> = {}
  for (const [k, prop] of Object.entries(cap.input_schema.properties)) {
    if (prop.default !== undefined) {
      init[k] = prop.default
    } else if (prop.type === 'boolean') {
      init[k] = false
    } else if (prop.type === 'number' || prop.type === 'integer') {
      init[k] = prop.minimum ?? 0
    } else {
      init[k] = ''
    }
  }
  quickFormData.value = init
})

function onQuickCobotletChange() {
  quickCapability.value = capabilitiesForQuick.value[0]?.[0] ?? ''
  quickResult.value = null
  quickError.value = null
  quickFormData.value = {}
}

function quickInputType(prop: JsonSchemaProperty): string {
  return prop.type === 'number' || prop.type === 'integer' ? 'number' : 'text'
}

async function quickSubmit() {
  if (!quickCobotlet.value || !quickCapability.value) return
  quickSubmitting.value = true
  quickResult.value = null
  quickError.value = null
  try {
    const entry = selectedQuickEntry.value
    const payload: SubmitTaskPayload = {
      cobotlet_alias: quickCobotlet.value,
      capability_name: quickCapability.value,
      device_id: entry?.[1].deviceId,
      body: quickSchema.value?.properties ? quickFormData.value : undefined,
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
                :is="d.state === 'ready' || d.state === 'busy' ? CircleDot : Circle"
                class="w-3.5 h-3.5 shrink-0"
                :class="d.state === 'ready' || d.state === 'busy' ? 'text-green-500' : 'text-muted-foreground'"
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
          <!-- cobotlet selector -->
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

          <!-- capability selector -->
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
            <!-- description + endpoint hint -->
            <p v-if="selectedQuickCapability?.description" class="mt-1 text-xs text-muted-foreground">
              {{ selectedQuickCapability.description }}
            </p>
            <p v-if="selectedQuickCapability" class="mt-0.5 text-xs text-muted-foreground font-mono">
              {{ selectedQuickCapability.method }} {{ selectedQuickCapability.path }}
            </p>
          </div>

          <!-- schema-driven params -->
          <div v-if="quickSchema?.properties">
            <label class="block text-xs text-muted-foreground mb-1.5">参数</label>
            <div class="space-y-2 rounded-md border border-border p-2.5">
              <div
                v-for="(prop, key) in quickSchema.properties"
                :key="key"
                class="grid grid-cols-5 gap-2 items-center"
              >
                <div class="col-span-2">
                  <span class="text-xs font-mono">{{ key }}</span>
                  <span v-if="quickSchema.required?.includes(String(key))" class="text-destructive text-xs ml-0.5">*</span>
                </div>
                <div class="col-span-3">
                  <!-- boolean -->
                  <div v-if="prop.type === 'boolean'" class="flex items-center gap-1.5">
                    <input
                      type="checkbox"
                      :checked="!!quickFormData[key]"
                      class="rounded"
                      @change="quickFormData[key] = ($event.target as HTMLInputElement).checked"
                    />
                    <span class="text-xs text-muted-foreground">{{ quickFormData[key] ? 'true' : 'false' }}</span>
                  </div>
                  <!-- enum -->
                  <select
                    v-else-if="prop.enum"
                    v-model="quickFormData[key]"
                    class="w-full rounded border border-border bg-transparent px-2 py-1 text-xs focus:outline-none focus:ring-1 focus:ring-ring"
                  >
                    <option v-for="opt in prop.enum" :key="String(opt)" :value="opt">{{ opt }}</option>
                  </select>
                  <!-- number/integer -->
                  <input
                    v-else-if="prop.type === 'number' || prop.type === 'integer'"
                    :type="quickInputType(prop)"
                    v-model.number="quickFormData[key]"
                    :min="prop.minimum"
                    :max="prop.maximum"
                    :placeholder="String(key)"
                    class="w-full rounded border border-border bg-transparent px-2 py-1 text-xs focus:outline-none focus:ring-1 focus:ring-ring"
                  />
                  <!-- string -->
                  <input
                    v-else
                    type="text"
                    v-model="quickFormData[key]"
                    :placeholder="prop.description ?? String(key)"
                    class="w-full rounded border border-border bg-transparent px-2 py-1 text-xs focus:outline-none focus:ring-1 focus:ring-ring"
                  />
                </div>
              </div>
            </div>
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
          <template v-else-if="quickResult">
            <pre class="rounded bg-muted text-xs p-2 overflow-auto max-h-28 whitespace-pre-wrap">{{ JSON.stringify(quickResult.body, null, 2) }}</pre>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>
