<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { RefreshCw, Radio } from '@lucide/vue'
import { listMcpSessions, type McpSession } from '../../api'

const sessions = ref<McpSession[]>([])
const loading = ref(false)
const refreshing = ref(false)
const error = ref<string | null>(null)
let timer: ReturnType<typeof setInterval> | null = null

/** Silent poll (15s background refresh) — never toggles the spinner. */
async function poll() {
  try {
    sessions.value = await listMcpSessions()
    error.value = null
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function load() {
  loading.value = true
  error.value = null
  try {
    sessions.value = await listMcpSessions()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

/** Manual refresh button — only this one shows the spinner. */
async function refresh() {
  refreshing.value = true
  try {
    await poll()
  } finally {
    refreshing.value = false
  }
}

function formatRelativeTime(ts: string): string {
  const diff = Date.now() - new Date(ts).getTime()
  if (diff < 0) return '刚刚'
  if (diff < 60_000) return `${Math.floor(diff / 1000)}秒前`
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}分钟前`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}小时前`
  return `${Math.floor(diff / 86_400_000)}天前`
}

function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id
}

onMounted(() => {
  load()
  timer = setInterval(poll, 15_000)
})
onUnmounted(() => {
  if (timer) clearInterval(timer)
})
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <p class="text-xs text-muted-foreground">活跃会话（5 分钟内有活动；每 15 秒自动刷新）</p>
      <button
        :disabled="refreshing"
        class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-xs hover:bg-muted disabled:opacity-50 transition-colors"
        @click="refresh"
      >
        <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': refreshing }" />
        刷新
      </button>
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div class="rounded-lg border border-border bg-card overflow-hidden">
      <div v-if="sessions.length === 0 && !loading" class="p-6 text-center text-sm text-muted-foreground">
        <Radio class="w-6 h-6 mx-auto mb-2 opacity-40" />
        当前没有活跃会话
      </div>
      <table v-else class="w-full text-sm">
        <thead>
          <tr class="border-b border-border text-left text-xs text-muted-foreground">
            <th class="px-3 py-2 font-medium">会话 ID</th>
            <th class="px-3 py-2 font-medium">用户</th>
            <th class="px-3 py-2 font-medium">传输</th>
            <th class="px-3 py-2 font-medium">连接于</th>
            <th class="px-3 py-2 font-medium">最近活动</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="s in sessions" :key="s.session_id" class="hover:bg-muted/40">
            <td class="px-3 py-2 font-mono text-xs" :title="s.session_id">{{ shortId(s.session_id) }}</td>
            <td class="px-3 py-2 font-mono text-xs">{{ s.subject || '—' }}</td>
            <td class="px-3 py-2">
              <span class="inline-block rounded bg-muted px-1.5 py-0.5 text-[11px] uppercase">{{ s.transport }}</span>
            </td>
            <td class="px-3 py-2 text-xs text-muted-foreground">{{ formatRelativeTime(s.connected_at) }}</td>
            <td class="px-3 py-2 text-xs text-muted-foreground">{{ formatRelativeTime(s.last_seen) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
