<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { listDevices, submitTaskSync, submitTaskAsync, getTaskResult } from '../api'
import type { DeviceVO, TaskResult } from '@cobotweb/shared'

// ── State ──
const devices = ref<DeviceVO[]>([])
const cobotletAlias = ref('')
const deviceId = ref('')
const capabilityName = ref('')
const bodyText = ref('{}')
const taskId = ref(`t-${Date.now()}`)
const timeoutMs = ref(5000)
const waitMode = ref(true) // sync by default
const submitting = ref(false)
const result = ref<TaskResult | null>(null)
const error = ref<string | null>(null)
const pollTaskId = ref('')
const polling = ref(false)

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

// ── Methods ──
function onCobotletChange() {
  const first = devicesForCobotlet.value[0]
  if (first) {
    deviceId.value = first.device_id
    capabilityName.value = first.capabilities?.[0]?.name ?? ''
  } else {
    deviceId.value = ''
    capabilityName.value = ''
  }
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
    if (waitMode.value) {
      result.value = await submitTaskSync({
        cobotlet_alias: cobotletAlias.value,
        capability_name: capabilityName.value,
        device_id: deviceId.value || undefined,
        task_id: taskId.value || undefined,
        body,
        timeout_ms: timeoutMs.value,
      })
    } else {
      const res = await submitTaskAsync({
        cobotlet_alias: cobotletAlias.value,
        capability_name: capabilityName.value,
        device_id: deviceId.value || undefined,
        task_id: taskId.value || undefined,
        body,
        timeout_ms: timeoutMs.value,
      })
      pollTaskId.value = res.task_id
    }
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    submitting.value = false
  }
}

async function pollResult() {
  if (!pollTaskId.value) return
  polling.value = true
  error.value = null
  try {
    result.value = await getTaskResult(pollTaskId.value)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    polling.value = false
  }
}
</script>

<template>
  <div class="space-y-4">
    <h2 class="text-lg font-semibold">任务派发</h2>
    <p class="text-sm text-muted-foreground">
      向指定 cobotlet 上的设备派发任务，通过 MQTT 下发到端侧执行。
    </p>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <!-- Form -->
      <div class="rounded-lg border border-border bg-card p-4 space-y-3">
        <div>
          <label class="block text-sm mb-1">cobotlet</label>
          <select
            v-model="cobotletAlias"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            @change="onCobotletChange"
          >
            <option value="">（选择一个 cobotlet）</option>
            <option v-for="alias in cobotletAliases" :key="alias" :value="alias">
              {{ alias }}
            </option>
          </select>
        </div>

        <div>
          <label class="block text-sm mb-1">设备</label>
          <select
            v-model="deviceId"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option value="">（按能力自动选择）</option>
            <option
              v-for="d in devicesForCobotlet"
              :key="d.device_id"
              :value="d.device_id"
            >
              {{ d.device_id }} ({{ d.capabilities?.length ?? 0 }} 个能力)
            </option>
          </select>
        </div>

        <div>
          <label class="block text-sm mb-1">能力</label>
          <select
            v-if="selectedDevice?.capabilities?.length"
            v-model="capabilityName"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option
              v-for="c in selectedDevice.capabilities"
              :key="c.name"
              :value="c.name"
            >
              {{ c.name }} ({{ c.method }} {{ c.path }})
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
            <label class="block text-sm mb-1">Task ID</label>
            <input
              v-model="taskId"
              class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>
          <div>
            <label class="block text-sm mb-1">超时 (ms)</label>
            <input
              v-model.number="timeoutMs"
              type="number"
              class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>
        </div>

        <div>
          <label class="block text-sm mb-1">Body (JSON)</label>
          <textarea
            v-model="bodyText"
            rows="6"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 font-mono text-xs focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>

        <div class="flex items-center gap-4">
          <label class="flex items-center gap-2 text-sm">
            <input v-model="waitMode" type="checkbox" class="rounded" />
            同步等待结果
          </label>
        </div>

        <button
          :disabled="submitting || !cobotletAlias || !capabilityName"
          class="inline-flex items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
          @click="submit"
        >
          {{ submitting ? '派发中…' : '派发任务' }}
        </button>

        <!-- Polling -->
        <div v-if="pollTaskId && !waitMode" class="space-y-2 pt-2 border-t border-border">
          <p class="text-xs text-muted-foreground">
            异步任务 ID: <code class="font-mono">{{ pollTaskId }}</code>
          </p>
          <button
            :disabled="polling"
            class="inline-flex items-center justify-center rounded-md border border-border bg-transparent px-3 py-1 text-xs hover:bg-muted"
            @click="pollResult"
          >
            {{ polling ? '查询中…' : '查询结果' }}
          </button>
        </div>
      </div>

      <!-- Result -->
      <div class="rounded-lg border border-border bg-card p-4">
        <h2 class="text-lg font-semibold mb-2">结果</h2>
        <pre
          v-if="error"
          class="rounded bg-destructive/10 text-destructive p-3 text-xs whitespace-pre-wrap"
        >{{ error }}</pre>
        <pre
          v-else-if="result"
          class="rounded bg-muted p-3 text-xs whitespace-pre-wrap overflow-auto max-h-[28rem]"
        >{{ JSON.stringify(result, null, 2) }}</pre>
        <p v-else class="text-sm text-muted-foreground">尚未派发任务。</p>
      </div>
    </div>
  </div>
</template>
