<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { listDevices } from '../api'
import type { DeviceVO } from '@cobotweb/shared'

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
  const totalCapabilities = devices.value.reduce((sum, d) => sum + (d.capabilities?.length ?? 0), 0)
  return { total, online, offline: total - online, cobotlets, totalCapabilities }
})
</script>

<template>
  <div class="space-y-6">
    <h2 class="text-lg font-semibold">总览</h2>

    <div v-if="error" class="rounded-md border border-destructive p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div v-if="loading" class="text-sm text-muted-foreground">加载中…</div>

    <div v-else class="grid grid-cols-2 md:grid-cols-5 gap-4">
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="text-sm text-muted-foreground">cobotlet 数</div>
        <div class="text-2xl font-bold">{{ stats.cobotlets }}</div>
      </div>
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="text-sm text-muted-foreground">设备总数</div>
        <div class="text-2xl font-bold">{{ stats.total }}</div>
      </div>
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="text-sm text-muted-foreground">在线</div>
        <div class="text-2xl font-bold text-green-600">{{ stats.online }}</div>
      </div>
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="text-sm text-muted-foreground">离线</div>
        <div class="text-2xl font-bold text-muted-foreground">{{ stats.offline }}</div>
      </div>
      <div class="rounded-lg border border-border bg-card p-4">
        <div class="text-sm text-muted-foreground">能力总数</div>
        <div class="text-2xl font-bold">{{ stats.totalCapabilities }}</div>
      </div>
    </div>
  </div>
</template>
