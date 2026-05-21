<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { listDevices } from '../api'
import type { DeviceVO } from '@cobotweb/shared'

const devices = ref<DeviceVO[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const selectedCobotlet = ref<string>('')

onMounted(async () => {
  await refresh()
})

async function refresh() {
  loading.value = true
  error.value = null
  try {
    devices.value = await listDevices()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

const cobotletAliases = computed(() =>
  [...new Set(devices.value.map((d) => d.cobotlet_alias))].sort(),
)

const filteredDevices = computed(() =>
  selectedCobotlet.value
    ? devices.value.filter((d) => d.cobotlet_alias === selectedCobotlet.value)
    : devices.value,
)
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">所有 cobotlet 设备</h2>
      <div class="flex items-center gap-3">
        <select
          v-model="selectedCobotlet"
          class="rounded-md border border-border bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        >
          <option value="">全部 cobotlet</option>
          <option v-for="alias in cobotletAliases" :key="alias" :value="alias">
            {{ alias }}
          </option>
        </select>
        <button
          class="inline-flex items-center justify-center rounded-md border border-border bg-transparent px-3 py-1.5 text-sm hover:bg-muted"
          @click="refresh"
        >
          刷新
        </button>
      </div>
    </div>

    <div v-if="error" class="rounded-md border border-destructive p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div class="rounded-lg border border-border bg-card overflow-hidden">
      <table class="min-w-full text-sm">
        <thead>
          <tr class="text-left bg-muted">
            <th class="px-3 py-2">cobotlet</th>
            <th class="px-3 py-2">设备 ID</th>
            <th class="px-3 py-2">能力</th>
            <th class="px-3 py-2">在线</th>
            <th class="px-3 py-2">最后心跳</th>
            <th class="px-3 py-2">注册时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading">
            <td class="px-3 py-4 text-muted-foreground" colspan="6">加载中…</td>
          </tr>
          <tr v-else-if="filteredDevices.length === 0">
            <td class="px-3 py-4 text-muted-foreground" colspan="6">
              还没有设备发现。请确保 cobotlet 已连接 MQTT。
            </td>
          </tr>
          <tr
            v-for="d in filteredDevices"
            :key="`${d.cobotlet_alias}/${d.device_id}`"
            class="border-t border-border"
          >
            <td class="px-3 py-2 font-mono text-xs">{{ d.cobotlet_alias }}</td>
            <td class="px-3 py-2 font-mono text-xs">{{ d.device_id }}</td>
            <td class="px-3 py-2">
              <span
                v-for="c in d.capabilities"
                :key="c.name"
                class="mr-1 inline-block rounded bg-muted px-1.5 py-0.5 text-xs"
                :title="`${c.method} ${c.path}`"
              >{{ c.name }}</span>
            </td>
            <td class="px-3 py-2">
              <span :class="d.online ? 'text-green-600' : 'text-muted-foreground'">
                {{ d.online ? '● 在线' : '○ 离线' }}
              </span>
            </td>
            <td class="px-3 py-2 text-xs text-muted-foreground">{{ d.last_heartbeat }}</td>
            <td class="px-3 py-2 text-xs text-muted-foreground">{{ d.registered_at }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
