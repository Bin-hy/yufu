<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { Send, CheckCircle, XCircle, History } from '@lucide/vue'
import { dispatchTask, listDevices, type LocalDevice, type LocalTaskResult } from '../api'

interface HistoryEntry {
  taskId: string
  deviceId: string
  capabilityName: string
  result: LocalTaskResult | null
  error: string | null
  submittedAt: Date
}

const devices = ref<LocalDevice[]>([])
const deviceId = ref('')
const capabilityName = ref('')
const bodyText = ref('{}')
const timeoutMs = ref(5000)
const taskId = ref(`t-${Date.now()}`)
const submitting = ref(false)
const result = ref<LocalTaskResult | null>(null)
const error = ref<string | null>(null)
const taskHistory = ref<HistoryEntry[]>([])

const selectedDevice = computed(() => devices.value.find((d) => d.id === deviceId.value))

const resultStatusColor = computed(() => {
  if (error.value) return 'text-destructive'
  if (!result.value) return ''
  const s = result.value.status
  if (s >= 200 && s < 300) return 'text-green-600'
  if (s >= 400) return 'text-destructive'
  return 'text-amber-500'
})

onMounted(async () => {
  try {
    devices.value = await listDevices()
    const first = devices.value.find((d) => d.online)
    if (first) {
      deviceId.value = first.id
      capabilityName.value = first.capabilities[0]?.name ?? ''
    }
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
})

function generateTaskId() {
  taskId.value = `t-${Date.now()}`
}

async function submit() {
  error.value = null
  result.value = null
  let body: unknown
  try {
    body = bodyText.value.trim() ? JSON.parse(bodyText.value) : undefined
  } catch (e) {
    error.value = `body 不是合法 JSON: ${e instanceof Error ? e.message : String(e)}`
    return
  }
  submitting.value = true
  try {
    result.value = await dispatchTask({
      id: taskId.value,
      capability_name: capabilityName.value,
      device_id: deviceId.value || undefined,
      body,
      timeout_ms: timeoutMs.value,
    })
    addHistory(result.value, null)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
    addHistory(null, error.value)
  } finally {
    submitting.value = false
    generateTaskId()
  }
}

function addHistory(res: LocalTaskResult | null, err: string | null) {
  taskHistory.value.unshift({
    taskId: taskId.value,
    deviceId: deviceId.value,
    capabilityName: capabilityName.value,
    result: res,
    error: err,
    submittedAt: new Date(),
  })
  if (taskHistory.value.length > 20) taskHistory.value.pop()
}

function restoreHistory(entry: HistoryEntry) {
  deviceId.value = entry.deviceId
  capabilityName.value = entry.capabilityName
  result.value = entry.result
  error.value = entry.error
}

function historyOk(entry: HistoryEntry): boolean | null {
  if (entry.error) return false
  if (!entry.result) return null
  return entry.result.status >= 200 && entry.result.status < 300
}

function formatTime(d: Date): string {
  return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}
</script>

<template>
  <div class="space-y-4">
    <h2 class="text-lg font-semibold">任务派发</h2>

    <div class="grid grid-cols-1 xl:grid-cols-2 gap-4">
      <!-- form -->
      <div class="rounded-lg border border-border bg-card p-4 space-y-3">
        <div>
          <label class="block text-xs text-muted-foreground mb-1">设备</label>
          <select
            v-model="deviceId"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option value="">— 按能力自动选择 —</option>
            <option v-for="d in devices" :key="d.id" :value="d.id" :disabled="!d.online">
              {{ d.id }}{{ d.online ? '' : ' (离线)' }}
            </option>
          </select>
        </div>

        <div>
          <label class="block text-xs text-muted-foreground mb-1">能力</label>
          <select
            v-if="selectedDevice?.capabilities?.length"
            v-model="capabilityName"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option v-for="c in selectedDevice.capabilities" :key="c.name" :value="c.name">
              {{ c.name }} — {{ c.method }} {{ c.path }}
            </option>
          </select>
          <input
            v-else
            v-model="capabilityName"
            placeholder="例如 move_forward"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>

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

        <div>
          <label class="block text-xs text-muted-foreground mb-1">Body (JSON)</label>
          <textarea
            v-model="bodyText"
            rows="5"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 font-mono text-xs focus:outline-none focus:ring-2 focus:ring-ring resize-y"
          />
        </div>

        <button
          :disabled="submitting || !capabilityName"
          class="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity"
          @click="submit"
        >
          <Send class="w-3.5 h-3.5" />
          {{ submitting ? '派发中…' : '派发任务' }}
        </button>
      </div>

      <!-- right panel -->
      <div class="flex flex-col gap-4">
        <!-- result -->
        <div class="rounded-lg border border-border bg-card p-4 flex-1">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-sm font-medium">结果</h3>
            <span v-if="result" class="text-xs font-mono" :class="resultStatusColor">
              HTTP {{ result.status }}
            </span>
          </div>
          <pre v-if="error" class="rounded bg-destructive/10 text-destructive p-3 text-xs whitespace-pre-wrap">{{ error }}</pre>
          <pre v-else-if="result" class="rounded bg-muted p-3 text-xs whitespace-pre-wrap overflow-auto max-h-56">{{ JSON.stringify(result, null, 2) }}</pre>
          <p v-else class="text-sm text-muted-foreground">尚未派发任务。</p>
        </div>

        <!-- history -->
        <div class="rounded-lg border border-border bg-card p-4">
          <div class="flex items-center gap-2 mb-2">
            <History class="w-4 h-4 text-muted-foreground" />
            <h3 class="text-sm font-medium">历史记录</h3>
            <span class="text-xs text-muted-foreground">(本次会话)</span>
          </div>
          <div v-if="taskHistory.length === 0" class="text-xs text-muted-foreground">暂无历史记录</div>
          <ul v-else class="space-y-1 max-h-40 overflow-auto">
            <li
              v-for="entry in taskHistory"
              :key="`${entry.taskId}-${entry.submittedAt.getTime()}`"
              class="flex items-center gap-2 px-2 py-1.5 rounded hover:bg-muted cursor-pointer text-xs"
              @click="restoreHistory(entry)"
            >
              <component
                :is="historyOk(entry) === false ? XCircle : CheckCircle"
                class="w-3.5 h-3.5 shrink-0"
                :class="historyOk(entry) === false ? 'text-destructive' : 'text-green-500'"
              />
              <span class="font-mono truncate flex-1">{{ entry.capabilityName }}</span>
              <span class="text-muted-foreground shrink-0 truncate max-w-24">{{ entry.deviceId || '自动' }}</span>
              <span class="text-muted-foreground shrink-0">{{ formatTime(entry.submittedAt) }}</span>
            </li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>
