<script setup lang="ts">
import { onMounted, ref } from 'vue'
import UiButton from '@cobotweb/shared/ui/button'
import { listDevices, type LocalDevice } from '../api'

const devices = ref<LocalDevice[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

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

onMounted(refresh)
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">本机注册的设备</h2>
      <UiButton variant="outline" size="sm" @click="refresh">刷新</UiButton>
    </div>

    <div v-if="error" class="rounded-md border border-destructive p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div class="rounded-lg border border-border bg-card overflow-hidden">
      <table class="min-w-full text-sm">
        <thead>
          <tr class="text-left bg-muted">
            <th class="px-3 py-2">ID</th>
            <th class="px-3 py-2">Service URL</th>
            <th class="px-3 py-2">能力</th>
            <th class="px-3 py-2">在线</th>
            <th class="px-3 py-2">最后心跳</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="loading">
            <td class="px-3 py-4" colspan="5">加载中…</td>
          </tr>
          <tr v-else-if="devices.length === 0">
            <td class="px-3 py-4 text-muted-foreground" colspan="5">还没有设备注册到本 cobotlet。</td>
          </tr>
          <tr v-for="d in devices" :key="d.id" class="border-t border-border">
            <td class="px-3 py-2 font-mono text-xs">{{ d.id }}</td>
            <td class="px-3 py-2 font-mono text-xs">{{ d.service_url }}</td>
            <td class="px-3 py-2">
              <span
                v-for="c in d.capabilities"
                :key="c.name"
                class="mr-1 inline-block rounded bg-muted px-1.5 py-0.5 text-xs"
              >{{ c.name }}</span>
            </td>
            <td class="px-3 py-2">
              <span :class="d.online ? 'text-green-600' : 'text-muted-foreground'">
                {{ d.online ? '●' : '○' }}
              </span>
            </td>
            <td class="px-3 py-2 text-xs">{{ d.last_heartbeat }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
