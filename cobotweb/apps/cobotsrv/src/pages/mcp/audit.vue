<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { RefreshCw, CheckCircle2, XCircle, ChevronLeft, ChevronRight } from '@lucide/vue'
import { listMcpLogs, type McpCallLog } from '../../api'
import { formatTimestamp } from './utils'

const rows = ref<McpCallLog[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const page = ref(1)
const pageSize = 20
const totalPages = ref(1)

const fSubject = ref('')
const fTool = ref('')
const fResult = ref<'' | 'ok' | 'error'>('')

async function load() {
  loading.value = true
  error.value = null
  try {
    const res = await listMcpLogs({
      page_number: page.value,
      page_size: pageSize,
      subject: fSubject.value || undefined,
      tool: fTool.value || undefined,
      result: fResult.value || undefined,
    })
    rows.value = res.data_list
    totalPages.value = Math.max(1, res.total_pages)
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

function applyFilters() {
  page.value = 1
  load()
}

function go(delta: number) {
  const next = page.value + delta
  if (next < 1 || next > totalPages.value) return
  page.value = next
  load()
}

onMounted(load)
</script>

<template>
  <div class="space-y-4">
    <!-- filters -->
    <div class="flex items-center gap-2 flex-wrap">
      <input
        v-model="fSubject"
        type="text"
        placeholder="用户 (sub)"
        class="w-28 rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        @keyup.enter="applyFilters"
      />
      <input
        v-model="fTool"
        type="text"
        placeholder="工具名"
        class="w-36 rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        @keyup.enter="applyFilters"
      />
      <select
        v-model="fResult"
        class="rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        @change="applyFilters"
      >
        <option value="">全部结果</option>
        <option value="ok">成功</option>
        <option value="error">失败</option>
      </select>
      <button
        class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-2 text-sm hover:bg-muted transition-colors"
        @click="applyFilters"
      >
        筛选
      </button>
      <button
        :disabled="loading"
        class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-2 text-sm hover:bg-muted disabled:opacity-50 transition-colors ml-auto"
        @click="load"
      >
        <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': loading }" />
        刷新
      </button>
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div class="rounded-lg border border-border bg-card overflow-hidden">
      <div v-if="rows.length === 0 && !loading" class="p-6 text-center text-sm text-muted-foreground">
        暂无调用记录
      </div>
      <table v-else class="w-full text-sm">
        <thead>
          <tr class="border-b border-border text-left text-xs text-muted-foreground">
            <th class="px-3 py-2 font-medium">时间</th>
            <th class="px-3 py-2 font-medium">用户</th>
            <th class="px-3 py-2 font-medium">方法</th>
            <th class="px-3 py-2 font-medium">工具</th>
            <th class="px-3 py-2 font-medium">结果</th>
            <th class="px-3 py-2 font-medium text-right">耗时</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="r in rows" :key="r.id" class="hover:bg-muted/40">
            <td class="px-3 py-2 text-xs text-muted-foreground whitespace-nowrap">{{ formatTimestamp(r.created_at) }}</td>
            <td class="px-3 py-2 font-mono text-xs">{{ r.subject }}</td>
            <td class="px-3 py-2 font-mono text-xs">{{ r.method }}</td>
            <td class="px-3 py-2 font-mono text-xs">{{ r.tool_name ?? '—' }}</td>
            <td class="px-3 py-2">
              <span
                v-if="!r.error_msg"
                class="inline-flex items-center gap-1 rounded bg-green-500/10 px-1.5 py-0.5 text-[11px] text-green-600"
              >
                <CheckCircle2 class="w-3 h-3" /> ok
              </span>
              <span
                v-else
                class="inline-flex items-center gap-1 rounded bg-destructive/10 px-1.5 py-0.5 text-[11px] text-destructive"
                :title="r.error_msg"
              >
                <XCircle class="w-3 h-3" /> err
              </span>
            </td>
            <td class="px-3 py-2 text-xs text-muted-foreground text-right">{{ r.duration_ms }}ms</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- pagination -->
    <div class="flex items-center justify-end gap-2 text-sm">
      <button
        :disabled="page <= 1 || loading"
        class="inline-flex items-center rounded-md border border-border p-1.5 hover:bg-muted disabled:opacity-40 transition-colors"
        @click="go(-1)"
      >
        <ChevronLeft class="w-4 h-4" />
      </button>
      <span class="text-xs text-muted-foreground">{{ page }} / {{ totalPages }}</span>
      <button
        :disabled="page >= totalPages || loading"
        class="inline-flex items-center rounded-md border border-border p-1.5 hover:bg-muted disabled:opacity-40 transition-colors"
        @click="go(1)"
      >
        <ChevronRight class="w-4 h-4" />
      </button>
    </div>
  </div>
</template>
