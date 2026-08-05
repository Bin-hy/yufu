<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Send, Gamepad2, Terminal } from '@lucide/vue'
import { listDevices, submitTaskSync } from '../api'
import { useControlEngine } from '../composables/useControlEngine'
import DpadPanel from '../components/control/DpadPanel.vue'
import DualStickPanel from '../components/control/DualStickPanel.vue'
import ButtonMatrixPanel from '../components/control/ButtonMatrixPanel.vue'
import FeedbackLog from '../components/control/FeedbackLog.vue'
import type { DeviceVO, Capability, ControlPreset, JsonSchemaProperty, TaskResult } from '@cobotweb/shared'

type Mode = 'control' | 'advanced'

const devices = ref<DeviceVO[]>([])
const cobotletAlias = ref('')
const deviceId = ref('')
const mode = ref<Mode>('control')
const loading = ref(false)
const error = ref<string | null>(null)

// Advanced mode state
const capabilityName = ref('')
const bodyText = ref('{}')
const formData = ref<Record<string, unknown>>({})
const timeoutMs = ref(5000)
const submitting = ref(false)
const result = ref<TaskResult | null>(null)
const advError = ref<string | null>(null)

// Derived
const onlineDevices = computed(() => devices.value.filter((d) => d.state === 'ready' || d.online))
const cobotletAliases = computed(() => [...new Set(onlineDevices.value.map((d) => d.cobotlet_alias))].sort())
const devicesForCobotlet = computed(() => onlineDevices.value.filter((d) => d.cobotlet_alias === cobotletAlias.value))
const selectedDevice = computed(() => devicesForCobotlet.value.find((d) => d.device_id === deviceId.value))
const preset = computed<ControlPreset | null>(() => (selectedDevice.value?.control_preset as ControlPreset) ?? null)
const hasPreset = computed(() => !!preset.value)
const selectedCapability = computed<Capability | undefined>(() => selectedDevice.value?.capabilities?.find((c) => c.name === capabilityName.value))
const schema = computed(() => selectedCapability.value?.input_schema ?? null)

// Control engine
const engine = useControlEngine(cobotletAlias, deviceId, preset)

// Auto-select mode based on preset
watch(hasPreset, (v) => { mode.value = v ? 'control' : 'advanced' })

// Init formData when capability changes
watch(selectedCapability, (cap) => {
  if (!cap?.input_schema?.properties) { formData.value = {}; return }
  const init: Record<string, unknown> = {}
  for (const [k, prop] of Object.entries(cap.input_schema.properties)) {
    if (prop.default !== undefined) init[k] = prop.default
    else if (prop.type === 'boolean') init[k] = false
    else if (prop.type === 'number' || prop.type === 'integer') init[k] = prop.minimum ?? 0
    else init[k] = ''
  }
  formData.value = init
})

onMounted(async () => {
  loading.value = true
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
  } finally {
    loading.value = false
  }
})

onUnmounted(() => engine.destroy())

function onCobotletChange() {
  const first = devicesForCobotlet.value[0]
  deviceId.value = first?.device_id ?? ''
  capabilityName.value = first?.capabilities?.[0]?.name ?? ''
}

function resolveBody(): unknown {
  if (schema.value?.properties) return formData.value
  try { return bodyText.value.trim() ? JSON.parse(bodyText.value) : undefined }
  catch { return undefined }
}

function validateBody(): string | null {
  if (schema.value?.properties) return null
  try { if (bodyText.value.trim()) JSON.parse(bodyText.value); return null }
  catch (e) { return `body 不是合法 JSON: ${e instanceof Error ? e.message : String(e)}` }
}

async function submitAdvanced() {
  advError.value = null
  result.value = null
  const bodyError = validateBody()
  if (bodyError) { advError.value = bodyError; return }
  submitting.value = true
  try {
    result.value = await submitTaskSync({
      cobotlet_alias: cobotletAlias.value,
      capability_name: capabilityName.value,
      device_id: deviceId.value || undefined,
      body: resolveBody(),
      timeout_ms: timeoutMs.value,
    })
  } catch (e: unknown) {
    advError.value = e instanceof Error ? e.message : String(e)
  } finally {
    submitting.value = false
  }
}

function inputType(prop: JsonSchemaProperty): string {
  return prop.type === 'number' || prop.type === 'integer' ? 'number' : 'text'
}
</script>

<template>
  <div class="space-y-4">
    <!-- Header + device selector -->
    <div class="flex flex-wrap items-center gap-3">
      <h2 class="text-lg font-semibold">控制台</h2>
      <select
        v-model="cobotletAlias"
        class="rounded-md border border-border bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        @change="onCobotletChange"
      >
        <option value="">— Cobotlet —</option>
        <option v-for="a in cobotletAliases" :key="a" :value="a">{{ a }}</option>
      </select>
      <select
        v-model="deviceId"
        class="rounded-md border border-border bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
      >
        <option value="">— 设备 —</option>
        <option v-for="d in devicesForCobotlet" :key="d.device_id" :value="d.device_id">
          {{ d.device_id }}
        </option>
      </select>

      <!-- Mode tabs -->
      <div class="ml-auto flex rounded-md border border-border overflow-hidden text-sm">
        <button
          class="px-3 py-1.5 flex items-center gap-1.5 transition-colors"
          :class="mode === 'control' ? 'bg-primary text-primary-foreground' : 'hover:bg-muted'"
          :disabled="!hasPreset"
          @click="mode = 'control'"
        >
          <Gamepad2 class="w-3.5 h-3.5" /> 控制面板
        </button>
        <button
          class="px-3 py-1.5 flex items-center gap-1.5 transition-colors"
          :class="mode === 'advanced' ? 'bg-primary text-primary-foreground' : 'hover:bg-muted'"
          @click="mode = 'advanced'"
        >
          <Terminal class="w-3.5 h-3.5" /> 高级模式
        </button>
      </div>
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div class="grid grid-cols-1 xl:grid-cols-3 gap-4">
      <!-- Main panel (2 cols) -->
      <div class="xl:col-span-2 rounded-lg border border-border bg-card p-5 min-h-[320px] flex items-center justify-center">
        <!-- Control panel mode -->
        <template v-if="mode === 'control' && preset">
          <DpadPanel v-if="preset.template === 'dpad'" :preset="preset" :active-keys="engine.activeKeys.value" :press="engine.press" :release="engine.release" />
          <DualStickPanel v-else-if="preset.template === 'dual_stick'" :preset="preset" :active-keys="engine.activeKeys.value" :press="engine.press" :release="engine.release" />
          <ButtonMatrixPanel v-else-if="preset.template === 'button_matrix'" :preset="preset" :active-keys="engine.activeKeys.value" :press="engine.press" :release="engine.release" />
        </template>

        <!-- No preset hint -->
        <div v-else-if="mode === 'control' && !preset" class="text-center text-muted-foreground text-sm">
          <Gamepad2 class="w-8 h-8 mx-auto mb-2 opacity-40" />
          <p>此设备未配置控制预设</p>
          <p class="text-xs mt-1">请切换到高级模式，或在 cobotlet.toml 中配置 [control_preset]</p>
        </div>

        <!-- Advanced mode -->
        <div v-else class="w-full space-y-3">
          <div>
            <label class="block text-xs text-muted-foreground mb-1">能力</label>
            <select
              v-if="selectedDevice?.capabilities?.length"
              v-model="capabilityName"
              class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            >
              <option v-for="c in selectedDevice.capabilities" :key="c.name" :value="c.name">{{ c.name }}</option>
            </select>
            <input v-else v-model="capabilityName" placeholder="能力名" class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring" />
            <p v-if="selectedCapability?.description" class="mt-1 text-xs text-muted-foreground">{{ selectedCapability.description }}</p>
          </div>

          <!-- Schema form or raw JSON -->
          <div v-if="schema?.properties" class="space-y-2 rounded-md border border-border p-3">
            <div v-for="(prop, key) in schema.properties" :key="key" class="grid grid-cols-3 gap-2 items-center">
              <span class="text-xs font-mono">{{ key }}</span>
              <div class="col-span-2">
                <input v-if="prop.type === 'boolean'" type="checkbox" :checked="!!formData[key]" @change="formData[key] = ($event.target as HTMLInputElement).checked" />
                <select v-else-if="prop.enum" v-model="formData[key]" class="w-full rounded-md border border-border bg-transparent px-2 py-1.5 text-sm">
                  <option v-for="opt in prop.enum" :key="String(opt)" :value="opt">{{ opt }}</option>
                </select>
                <input v-else :type="inputType(prop)" v-model="formData[key]" class="w-full rounded-md border border-border bg-transparent px-2 py-1.5 text-sm" />
              </div>
            </div>
          </div>
          <div v-else>
            <label class="block text-xs text-muted-foreground mb-1">Body (JSON)</label>
            <textarea v-model="bodyText" rows="4" class="w-full rounded-md border border-border bg-transparent px-3 py-2 font-mono text-xs resize-y focus:outline-none focus:ring-2 focus:ring-ring" />
          </div>

          <div class="flex items-center gap-3">
            <input v-model.number="timeoutMs" type="number" class="w-24 rounded-md border border-border bg-transparent px-2 py-1.5 text-sm" />
            <span class="text-xs text-muted-foreground">ms 超时</span>
            <button
              :disabled="submitting || !cobotletAlias || !capabilityName"
              class="ml-auto inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
              @click="submitAdvanced"
            >
              <Send class="w-3.5 h-3.5" /> {{ submitting ? '发送中…' : '发送' }}
            </button>
          </div>

          <!-- Result -->
          <pre v-if="advError" class="rounded bg-destructive/10 text-destructive p-3 text-xs whitespace-pre-wrap">{{ advError }}</pre>
          <pre v-else-if="result" class="rounded bg-muted p-3 text-xs whitespace-pre-wrap overflow-auto max-h-40">{{ JSON.stringify(result.body, null, 2) }}</pre>
        </div>
      </div>

      <!-- Feedback panel (1 col) -->
      <div>
        <FeedbackLog :log="engine.feedbackLog.value" />
      </div>
    </div>
  </div>
</template>



