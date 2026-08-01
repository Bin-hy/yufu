<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { CircleDot, Wrench, Radio, Activity, RefreshCw, CheckCircle2, XCircle, Cpu } from '@lucide/vue'
import { getMcpInfo, listMcpSessions, type McpInfo } from '../../api'
import { mcpEndpoint } from './utils'

const info = ref<McpInfo | null>(null)
const sessionCount = ref(0)
const loading = ref(false)
const error = ref<string | null>(null)

// health self-check
const checking = ref(false)
const checkOk = ref<boolean | null>(null)
const checkMsg = ref('')

const endpoint = computed(mcpEndpoint)

async function load() {
  loading.value = true
  error.value = null
  try {
    const [i, s] = await Promise.all([getMcpInfo(), listMcpSessions()])
    info.value = i
    sessionCount.value = s.length
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function healthCheck() {
  checking.value = true
  checkOk.value = null
  try {
    const i = await getMcpInfo()
    info.value = i
    checkOk.value = true
    checkMsg.value = `协议 ${i.protocol_version} · ${i.online_tools} 个在线工具`
  } catch (e: unknown) {
    checkOk.value = false
    checkMsg.value = e instanceof Error ? e.message : String(e)
  } finally {
    checking.value = false
  }
}

const successRate = computed(() => {
  if (!info.value || info.value.calls_today === 0) return '—'
  return `${Math.round((info.value.success_today / info.value.calls_today) * 100)}%`
})

const statCards = computed(() => [
  { label: '协议版本', value: info.value?.protocol_version ?? '—', icon: CircleDot, color: 'text-foreground' },
  { label: '在线工具', value: info.value?.online_tools ?? '—', icon: Wrench, color: 'text-amber-500' },
  { label: '在线设备', value: info.value?.online_devices ?? '—', icon: Cpu, color: 'text-blue-500' },
  { label: '活跃会话', value: sessionCount.value, icon: Radio, color: 'text-green-600' },
  {
    label: '今日调用',
    value: info.value ? `${info.value.calls_today} (${successRate.value})` : '—',
    icon: Activity,
    color: 'text-foreground',
  },
])

// status badge reflects the actual load state — no hardcoded "运行中" when the
// info request failed
const statusBadge = computed(() => {
  if (error.value) return { label: '异常', cls: 'bg-destructive/10 text-destructive' }
  if (!info.value) return { label: '检查中…', cls: 'bg-muted text-muted-foreground' }
  return { label: '运行中', cls: 'bg-green-500/10 text-green-600' }
})

onMounted(load)
</script>

<template>
  <div class="space-y-5">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2 text-sm">
        <span
          class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium"
          :class="statusBadge.cls"
        >
          <CircleDot class="w-3 h-3" /> {{ statusBadge.label }}
        </span>
      </div>
      <button
        :disabled="checking"
        class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-xs hover:bg-muted disabled:opacity-50 transition-colors"
        @click="healthCheck"
      >
        <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': checking }" />
        健康自检
      </button>
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div
      v-if="checkOk !== null"
      class="rounded-md border p-3 text-sm flex items-center gap-2"
      :class="checkOk ? 'border-green-500/40 bg-green-500/5 text-green-700' : 'border-destructive/40 bg-destructive/5 text-destructive'"
    >
      <component :is="checkOk ? CheckCircle2 : XCircle" class="w-4 h-4 shrink-0" />
      {{ checkOk ? '自检通过' : '自检失败' }} — {{ checkMsg }}
    </div>

    <!-- stat cards -->
    <div class="grid grid-cols-2 lg:grid-cols-5 gap-4">
      <div
        v-for="card in statCards"
        :key="card.label"
        class="rounded-lg border border-border bg-card p-4 flex items-start gap-3"
      >
        <div class="mt-0.5 p-2 rounded-md bg-muted">
          <component :is="card.icon" class="w-4 h-4" :class="card.color" />
        </div>
        <div class="min-w-0">
          <div class="text-xs text-muted-foreground mb-0.5">{{ card.label }}</div>
          <div class="text-xl font-bold leading-tight truncate" :class="card.color">
            <span v-if="loading" class="text-muted-foreground text-base">—</span>
            <span v-else>{{ card.value }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- read-only runtime params -->
    <div class="rounded-lg border border-border bg-card p-4">
      <h3 class="text-sm font-medium mb-3">运行参数（只读）</h3>
      <dl class="grid grid-cols-1 sm:grid-cols-[8rem_1fr] gap-y-2 gap-x-4 text-sm">
        <dt class="text-muted-foreground">端点</dt>
        <dd class="font-mono text-xs break-all">POST / GET&nbsp;&nbsp;{{ endpoint }}</dd>
        <dt class="text-muted-foreground">调用超时</dt>
        <dd class="font-mono text-xs">call_timeout_ms = {{ info?.call_timeout_ms ?? '—' }}</dd>
        <dt class="text-muted-foreground">鉴权</dt>
        <dd class="text-xs">OAuth 2.1 + PKCE（Bearer，aud=cobotsrv，需 <code class="font-mono">mcp:access</code> 权限）</dd>
        <dt class="text-muted-foreground">修改方式</dt>
        <dd class="text-xs text-muted-foreground">编辑配置文件的 <code class="font-mono">[mcp]</code> 段后重启服务（面板内不可编辑）</dd>
      </dl>
    </div>
  </div>
</template>
