<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import UiButton from '@cobotweb/shared/ui/button'
import UiInput from '@cobotweb/shared/ui/input'
import UiSelect from '@cobotweb/shared/ui/select'
import { dispatchTask, listDevices, type LocalDevice, type LocalTaskResult } from '../api'

const devices = ref<LocalDevice[]>([])
const deviceId = ref('')
const capabilityName = ref('')
const bodyText = ref('{}')
const timeoutMs = ref(5000)
const taskId = ref(`t-${Date.now()}`)
const submitting = ref(false)
const result = ref<LocalTaskResult | null>(null)
const error = ref<string | null>(null)

const selectedDevice = computed(() => devices.value.find((d) => d.id === deviceId.value))

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
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
    <div class="rounded-lg border border-border bg-card p-4 space-y-3">
      <h2 class="text-lg font-semibold">派发本机任务（调试）</h2>

      <div>
        <label class="block text-sm mb-1">设备</label>
        <UiSelect v-model="deviceId" class="w-full">
          <option value="">（按能力自动选择）</option>
          <option v-for="d in devices" :key="d.id" :value="d.id" :disabled="!d.online">
            {{ d.id }} {{ d.online ? '' : '(离线)' }}
          </option>
        </UiSelect>
      </div>

      <div>
        <label class="block text-sm mb-1">能力名</label>
        <UiSelect v-if="selectedDevice" v-model="capabilityName" class="w-full">
          <option v-for="c in selectedDevice.capabilities" :key="c.name" :value="c.name">
            {{ c.name }} ({{ c.method }} {{ c.path }})
          </option>
        </UiSelect>
        <UiInput v-else v-model="capabilityName" placeholder="例如 move_forward" />
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-sm mb-1">task id</label>
          <UiInput v-model="taskId" />
        </div>
        <div>
          <label class="block text-sm mb-1">timeout (ms)</label>
          <UiInput v-model.number="timeoutMs" type="number" />
        </div>
      </div>

      <div>
        <label class="block text-sm mb-1">body (JSON)</label>
        <textarea
          v-model="bodyText"
          rows="6"
          class="w-full rounded-md border border-border bg-transparent px-3 py-2 font-mono text-xs focus:outline-none focus:ring-2 focus:ring-ring"
        />
      </div>

      <UiButton :disabled="submitting" @click="submit">
        {{ submitting ? '派发中…' : '派发任务' }}
      </UiButton>
    </div>

    <div class="rounded-lg border border-border bg-card p-4">
      <h2 class="text-lg font-semibold mb-2">结果</h2>
      <pre v-if="error" class="rounded bg-destructive/10 text-destructive p-3 text-xs whitespace-pre-wrap">{{ error }}</pre>
      <pre v-else-if="result" class="rounded bg-muted p-3 text-xs whitespace-pre-wrap">{{ JSON.stringify(result, null, 2) }}</pre>
      <p v-else class="text-sm text-muted-foreground">尚未派发任务。</p>
    </div>
  </div>
</template>
