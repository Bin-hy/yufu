<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Send, CheckCircle, XCircle, Clock, History } from '@lucide/vue'
import { listDevices, submitTaskSync, submitTaskAsync, getTaskResult } from '../api'
import type { Capability, DeviceVO, JsonSchemaProperty, TaskResult } from '@cobotweb/shared'

// ── Types ──
interface HistoryEntry {
  taskId: string
  cobotletAlias: string
  deviceId: string
  capabilityName: string
  result: TaskResult | null
  error: string | null
  submittedAt: Date
}

// ── State ──
const devices = ref<DeviceVO[]>([])
const cobotletAlias = ref('')
const deviceId = ref('')
const capabilityName = ref('')
const bodyText = ref('{}')
const formData = ref<Record<string, unknown>>({})
const taskId = ref(`t-${Date.now()}`)
const timeoutMs = ref(5000)
const waitMode = ref(true)
const submitting = ref(false)
const result = ref<TaskResult | null>(null)
const error = ref<string | null>(null)
const taskHistory = ref<HistoryEntry[]>([])

// async polling state
const asyncTaskId = ref('')
const asyncPolling = ref(false)
const asyncPollAttempts = ref(0)
let asyncPollTimer: ReturnType<typeof setTimeout> | null = null

// ── Derived ──
const selectedDevice = computed(() =>
  devices.value.find(
    (d) => d.cobotlet_alias === cobotletAlias.value && d.device_id === deviceId.value,
  ),
)
const onlineDevices = computed(() => devices.value.filter((d) => d.online))
const cobotletAliases = computed(() =>
  [...new Set(onlineDevices.value.map((d) => d.cobotlet_alias))].sort(),
)
const devicesForCobotlet = computed(() =>
  onlineDevices.value.filter((d) => d.cobotlet_alias === cobotletAlias.value),
)
const selectedCapability = computed<Capability | undefined>(() =>
  selectedDevice.value?.capabilities?.find((c) => c.name === capabilityName.value),
)
const schema = computed(() => selectedCapability.value?.input_schema ?? null)

const resultStatusColor = computed(() => {
  if (error.value) return 'text-destructive'
  if (!result.value) return ''
  const s = result.value.status
  if (s >= 200 && s < 300) return 'text-green-600'
  if (s >= 400) return 'text-destructive'
  return 'text-amber-500'
})

// ── Init formData when capability changes ──
watch(selectedCapability, (cap) => {
  if (!cap?.input_schema?.properties) {
    formData.value = {}
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
  formData.value = init
})

// ── Init ──
onMounted(async () => {
  try {
    devices.value = await listDevices()
    const first = onlineDevices.value[0]
    if (first) {
      cobotletAlias.value = first.cobotlet_alias
      deviceId.value = first.device_id
      capabilityName.value = first.capabilities?.[0]?.name ?? ''
    }
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
})

onUnmounted(() => {
  if (asyncPollTimer) clearTimeout(asyncPollTimer)
})

// ── Methods ──
function onCobotletChange() {
  const first = devicesForCobotlet.value[0]
  deviceId.value = first?.device_id ?? ''
  capabilityName.value = first?.capabilities?.[0]?.name ?? ''
}

function generateTaskId() {
  taskId.value = `t-${Date.now()}`
}

function resolveBody(): unknown {
  if (schema.value?.properties) {
    return formData.value
  }
  try {
    return bodyText.value.trim() ? JSON.parse(bodyText.value) : undefined
  } catch {
    return undefined
  }
}

function validateBody(): string | null {
  if (schema.value?.properties) return null
  try {
    if (bodyText.value.trim()) JSON.parse(bodyText.value)
    return null
  } catch (e) {
    return `body 不是合法 JSON: ${e instanceof Error ? e.message : String(e)}`
  }
}

async function submit() {
  error.value = null
  result.value = null
  asyncTaskId.value = ''

  const bodyError = validateBody()
  if (bodyError) {
    error.value = bodyError
    return
  }

  submitting.value = true
  const payload = {
    cobotlet_alias: cobotletAlias.value,
    capability_name: capabilityName.value,
    device_id: deviceId.value || undefined,
    task_id: taskId.value || undefined,
    body: resolveBody(),
    timeout_ms: timeoutMs.value,
  }

  try {
    if (waitMode.value) {
      result.value = await submitTaskSync(payload)
      addHistory(result.value, null)
    } else {
      const res = await submitTaskAsync(payload)
      asyncTaskId.value = res.task_id
      startAutoPoll(res.task_id)
    }
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
    addHistory(null, error.value)
  } finally {
    submitting.value = false
    generateTaskId()
  }
}

function addHistory(res: TaskResult | null, err: string | null) {
  taskHistory.value.unshift({
    taskId: taskId.value,
    cobotletAlias: cobotletAlias.value,
    deviceId: deviceId.value,
    capabilityName: capabilityName.value,
    result: res,
    error: err,
    submittedAt: new Date(),
  })
  if (taskHistory.value.length > 20) taskHistory.value.pop()
}

function startAutoPoll(tid: string) {
  asyncPolling.value = true
  asyncPollAttempts.value = 0
  schedulePoll(tid)
}

function schedulePoll(tid: string) {
  asyncPollTimer = setTimeout(() => void doPoll(tid), 2000)
}

async function doPoll(tid: string) {
  asyncPollAttempts.value++
  try {
    const r = await getTaskResult(tid)
    result.value = r
    asyncPolling.value = false
    addHistory(r, null)
  } catch (e: unknown) {
    if (asyncPollAttempts.value < 30) {
      schedulePoll(tid)
    } else {
      asyncPolling.value = false
      error.value = `轮询超时（${asyncPollAttempts.value} 次）: ${e instanceof Error ? e.message : String(e)}`
      addHistory(null, error.value)
    }
  }
}

function restoreHistory(entry: HistoryEntry) {
  cobotletAlias.value = entry.cobotletAlias
  deviceId.value = entry.deviceId
  capabilityName.value = entry.capabilityName
  result.value = entry.result
  error.value = entry.error
  asyncTaskId.value = ''
}

function historyStatus(entry: HistoryEntry): { ok: boolean | null } {
  if (entry.error) return { ok: false }
  if (!entry.result) return { ok: null }
  return { ok: entry.result.status >= 200 && entry.result.status < 300 }
}

function formatTime(d: Date): string {
  return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function inputType(prop: JsonSchemaProperty): string {
  if (prop.type === 'number' || prop.type === 'integer') return 'number'
  return 'text'
}
</script>

<template>
  <div class="space-y-4">
    <h2 class="text-lg font-semibold">任务派发</h2>

    <div class="grid grid-cols-1 xl:grid-cols-2 gap-4">
      <!-- ── Form ── -->
      <div class="rounded-lg border border-border bg-card p-4 space-y-3">
        <!-- cobotlet -->
        <div>
          <label class="block text-xs text-muted-foreground mb-1">Cobotlet</label>
          <select
            v-model="cobotletAlias"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            @change="onCobotletChange"
          >
            <option value="">— 选择 cobotlet —</option>
            <option v-for="alias in cobotletAliases" :key="alias" :value="alias">{{ alias }}</option>
          </select>
        </div>

        <!-- device -->
        <div>
          <label class="block text-xs text-muted-foreground mb-1">设备</label>
          <select
            v-model="deviceId"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option value="">— 按能力自动选择 —</option>
            <option v-for="d in devicesForCobotlet" :key="d.device_id" :value="d.device_id">
              {{ d.device_id }} ({{ d.capabilities?.length ?? 0 }} 个能力)
            </option>
          </select>
        </div>

        <!-- capability -->
        <div>
          <label class="block text-xs text-muted-foreground mb-1">能力</label>
          <select
            v-if="selectedDevice?.capabilities?.length"
            v-model="capabilityName"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option v-for="c in selectedDevice.capabilities" :key="c.name" :value="c.name">
              {{ c.name }}
            </option>
          </select>
          <input
            v-else
            v-model="capabilityName"
            placeholder="例如 move_forward"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          />
          <!-- capability description + endpoint hint -->
          <p v-if="selectedCapability?.description" class="mt-1 text-xs text-muted-foreground">
            {{ selectedCapability.description }}
          </p>
          <p
            v-if="selectedCapability"
            class="mt-0.5 text-xs text-muted-foreground font-mono"
          >
            {{ selectedCapability.method }} {{ selectedCapability.path }}
          </p>
        </div>

        <!-- task id + timeout -->
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-xs text-muted-foreground mb-1">Task ID</label>
            <input
              v-model="taskId"
              class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>
          <div>
            <label class="block text-xs text-muted-foreground mb-1">超时 (ms)</label>
            <input
              v-model.number="timeoutMs"
              type="number"
              class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>
        </div>

        <!-- ── Body: schema-driven form or raw textarea ── -->
        <div v-if="schema?.properties">
          <label class="block text-xs text-muted-foreground mb-2">参数</label>
          <div class="space-y-2 rounded-md border border-border p-3">
            <div
              v-for="(prop, key) in schema.properties"
              :key="key"
              class="grid grid-cols-3 gap-2 items-start"
            >
              <!-- field name + description -->
              <div class="pt-1">
                <span class="text-xs font-mono">{{ key }}</span>
                <span
                  v-if="schema.required?.includes(String(key))"
                  class="text-destructive text-xs ml-0.5"
                >*</span>
                <p v-if="prop.description" class="text-xs text-muted-foreground leading-tight mt-0.5">
                  {{ prop.description }}
                </p>
              </div>

              <!-- input control based on type -->
              <div class="col-span-2">
                <!-- boolean → checkbox -->
                <div v-if="prop.type === 'boolean'" class="flex items-center gap-2 pt-1">
                  <input
                    type="checkbox"
                    :checked="!!formData[key]"
                    class="rounded"
                    @change="formData[key] = ($event.target as HTMLInputElement).checked"
                  />
                  <span class="text-xs text-muted-foreground">{{ formData[key] ? 'true' : 'false' }}</span>
                </div>

                <!-- enum → select -->
                <select
                  v-else-if="prop.enum"
                  v-model="formData[key]"
                  class="w-full rounded-md border border-border bg-transparent px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
                >
                  <option v-for="opt in prop.enum" :key="String(opt)" :value="opt">
                    {{ opt }}
                  </option>
                </select>

                <!-- number / integer -->
                <input
                  v-else-if="prop.type === 'number' || prop.type === 'integer'"
                  :type="inputType(prop)"
                  v-model.number="formData[key]"
                  :min="prop.minimum"
                  :max="prop.maximum"
                  class="w-full rounded-md border border-border bg-transparent px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
                />

                <!-- string (default) -->
                <input
                  v-else
                  type="text"
                  v-model="formData[key]"
                  :placeholder="prop.description ?? String(key)"
                  class="w-full rounded-md border border-border bg-transparent px-2 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
                />
              </div>
            </div>
          </div>
        </div>

        <!-- fallback: raw JSON textarea (no schema) -->
        <div v-else>
          <label class="block text-xs text-muted-foreground mb-1">Body (JSON)</label>
          <textarea
            v-model="bodyText"
            rows="5"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 font-mono text-xs focus:outline-none focus:ring-2 focus:ring-ring resize-y"
          />
        </div>

        <!-- sync toggle + submit -->
        <div class="flex items-center gap-4">
          <label class="flex items-center gap-2 text-sm cursor-pointer">
            <input v-model="waitMode" type="checkbox" class="rounded" />
            同步等待结果
          </label>
        </div>

        <button
          :disabled="submitting || asyncPolling || !cobotletAlias || !capabilityName"
          class="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity"
          @click="submit"
        >
          <Send class="w-3.5 h-3.5" />
          {{ submitting ? '派发中…' : asyncPolling ? '等待结果…' : '派发任务' }}
        </button>

        <!-- async polling status -->
        <div v-if="asyncPolling" class="flex items-center gap-2 text-xs text-muted-foreground pt-1">
          <Clock class="w-3.5 h-3.5 animate-pulse" />
          轮询中（第 {{ asyncPollAttempts }} 次）… Task ID:
          <code class="font-mono">{{ asyncTaskId }}</code>
        </div>
      </div>

      <!-- ── Right panel ── -->
      <div class="flex flex-col gap-4">
        <!-- result -->
        <div class="rounded-lg border border-border bg-card p-4 flex-1">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-sm font-medium">结果</h3>
            <span v-if="result" class="text-xs font-mono" :class="resultStatusColor">
              HTTP {{ result.status }}
            </span>
          </div>

          <pre
            v-if="error"
            class="rounded bg-destructive/10 text-destructive p-3 text-xs whitespace-pre-wrap"
          >{{ error }}</pre>
          <template v-else-if="result">
            <!-- body prominently -->
            <pre class="rounded bg-muted p-3 text-xs whitespace-pre-wrap overflow-auto max-h-48">{{ JSON.stringify(result.body, null, 2) }}</pre>
            <!-- full response collapsible -->
            <details class="mt-2">
              <summary class="text-xs text-muted-foreground cursor-pointer select-none hover:text-foreground">
                完整响应
              </summary>
              <pre class="mt-1 rounded bg-muted/50 p-3 text-xs whitespace-pre-wrap overflow-auto max-h-40">{{ JSON.stringify(result, null, 2) }}</pre>
            </details>
          </template>
          <p v-else class="text-sm text-muted-foreground">尚未派发任务。</p>
        </div>

        <!-- history -->
        <div class="rounded-lg border border-border bg-card p-4">
          <div class="flex items-center gap-2 mb-2">
            <History class="w-4 h-4 text-muted-foreground" />
            <h3 class="text-sm font-medium">历史记录</h3>
            <span class="text-xs text-muted-foreground">(本次会话)</span>
          </div>

          <div v-if="taskHistory.length === 0" class="text-xs text-muted-foreground">
            暂无历史记录
          </div>

          <ul v-else class="space-y-1 max-h-48 overflow-auto">
            <li
              v-for="entry in taskHistory"
              :key="`${entry.taskId}-${entry.submittedAt.getTime()}`"
              class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-muted cursor-pointer text-xs"
              @click="restoreHistory(entry)"
            >
              <component
                :is="historyStatus(entry).ok === null ? Clock : historyStatus(entry).ok ? CheckCircle : XCircle"
                class="w-3.5 h-3.5 shrink-0"
                :class="{
                  'text-green-500': historyStatus(entry).ok === true,
                  'text-destructive': historyStatus(entry).ok === false,
                  'text-muted-foreground': historyStatus(entry).ok === null,
                }"
              />
              <span class="font-mono truncate flex-1">{{ entry.capabilityName }}</span>
              <span class="text-muted-foreground shrink-0">{{ entry.cobotletAlias }}</span>
              <span class="text-muted-foreground shrink-0">{{ formatTime(entry.submittedAt) }}</span>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>
