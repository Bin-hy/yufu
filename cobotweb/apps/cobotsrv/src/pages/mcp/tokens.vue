<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  KeyRound,
  Plus,
  RefreshCw,
  Pencil,
  Trash2,
  Copy,
  Check,
  Eye,
  EyeOff,
  X,
  ShieldAlert,
} from '@lucide/vue'
import {
  listMcpTokens,
  issueMcpToken,
  renameMcpToken,
  refreshMcpToken,
  deleteMcpToken,
  type McpTokenInfo,
  type McpTokenResponse,
} from '../../auth'
import { getMcpInfo } from '../../api'
import { formatTimestamp, maskToken, ttlOptions, useCopy } from './utils'

/** Focus the dialog overlay on open so Escape/keyboard nav works immediately. */
const vFocus = {
  mounted: (el: HTMLElement) => el.focus(),
}

const tokens = ref<McpTokenInfo[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

// Audience must come from the server (== cobotsrv's expected_audience); a token
// minted for the wrong aud is rejected on verification. Creation stays disabled
// until it loads successfully.
const audience = ref('')
const audienceReady = ref(false)

async function load() {
  loading.value = true
  error.value = null
  try {
    tokens.value = await listMcpTokens()
  } catch (e: unknown) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  try {
    const info = await getMcpInfo()
    if (info.audience) {
      audience.value = info.audience
      audienceReady.value = true
    }
  } catch {
    /* leave audience unset; creation stays disabled until a reload succeeds */
  }
  await load()
})

// ── shared: the once-only token reveal dialog (after create / refresh) ──
const result = ref<McpTokenResponse | null>(null)
const resultRevealed = ref(false)
const { copied, copyFailed, copy } = useCopy()
const maskedResult = computed(() => {
  const t = result.value?.access_token
  if (!t) return ''
  return resultRevealed.value ? t : maskToken(t)
})
function copyResult() {
  if (!result.value) return
  void copy('result', result.value.access_token)
}
function closeResult() {
  result.value = null
  resultRevealed.value = false
}

// ── create ──
const showCreate = ref(false)
const newName = ref('')
const newTtl = ref(30)
const creating = ref(false)
const createError = ref<string | null>(null)

function openCreate() {
  newName.value = ''
  newTtl.value = 30
  createError.value = null
  showCreate.value = true
}
async function submitCreate() {
  if (!audienceReady.value) return
  creating.value = true
  createError.value = null
  try {
    const res = await issueMcpToken(audience.value, newTtl.value, newName.value.trim() || undefined)
    showCreate.value = false
    result.value = res
    resultRevealed.value = false
    await load()
  } catch (e: unknown) {
    createError.value = e instanceof Error ? e.message : String(e)
  } finally {
    creating.value = false
  }
}

// ── rename ──
const renameTarget = ref<McpTokenInfo | null>(null)
const renameName = ref('')
const renaming = ref(false)
const renameError = ref<string | null>(null)

function openRename(t: McpTokenInfo) {
  renameTarget.value = t
  renameName.value = t.name
  renameError.value = null
}
async function submitRename() {
  if (!renameTarget.value) return
  const name = renameName.value.trim()
  if (!name) {
    renameError.value = '名称不能为空'
    return
  }
  renaming.value = true
  renameError.value = null
  try {
    await renameMcpToken(renameTarget.value.id, name)
    renameTarget.value = null
    await load()
  } catch (e: unknown) {
    renameError.value = e instanceof Error ? e.message : String(e)
  } finally {
    renaming.value = false
  }
}

// ── refresh (rotate) ──
const refreshTarget = ref<McpTokenInfo | null>(null)
const refreshTtl = ref(30)
const refreshing = ref(false)
const refreshError = ref<string | null>(null)

function openRefresh(t: McpTokenInfo) {
  refreshTarget.value = t
  refreshTtl.value = 30
  refreshError.value = null
}
async function submitRefresh() {
  if (!refreshTarget.value) return
  refreshing.value = true
  refreshError.value = null
  try {
    const res = await refreshMcpToken(refreshTarget.value.id, refreshTtl.value)
    refreshTarget.value = null
    result.value = res
    resultRevealed.value = false
    await load()
  } catch (e: unknown) {
    refreshError.value = e instanceof Error ? e.message : String(e)
  } finally {
    refreshing.value = false
  }
}

// ── delete (revoke) ──
const deleteTarget = ref<McpTokenInfo | null>(null)
const deleting = ref(false)
const deleteError = ref<string | null>(null)

async function confirmDelete() {
  if (!deleteTarget.value) return
  deleting.value = true
  deleteError.value = null
  try {
    await deleteMcpToken(deleteTarget.value.id)
    deleteTarget.value = null
    await load()
  } catch (e: unknown) {
    deleteError.value = e instanceof Error ? e.message : String(e)
  } finally {
    deleting.value = false
  }
}

function isExpired(ts: string): boolean {
  return new Date(ts).getTime() < Date.now()
}
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <p class="text-xs text-muted-foreground">
        长期访问令牌：携带你的权限快照，删除 / 刷新会在数秒内于服务端生效（吊销）。
      </p>
      <div class="flex items-center gap-2">
        <button
          :disabled="loading"
          class="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-xs hover:bg-muted disabled:opacity-50 transition-colors"
          @click="load"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': loading }" />
          刷新
        </button>
        <button
          :disabled="!audienceReady"
          :title="audienceReady ? '' : 'audience 尚未加载，暂不可生成'"
          class="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity"
          @click="openCreate"
        >
          <Plus class="w-3.5 h-3.5" />
          新建令牌
        </button>
      </div>
    </div>

    <div v-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
      {{ error }}
    </div>

    <div class="rounded-lg border border-border bg-card overflow-hidden">
      <div v-if="tokens.length === 0 && !loading" class="p-6 text-center text-sm text-muted-foreground">
        <KeyRound class="w-6 h-6 mx-auto mb-2 opacity-40" />
        还没有令牌，点击「新建令牌」生成第一个
      </div>
      <table v-else class="w-full text-sm">
        <thead>
          <tr class="border-b border-border text-left text-xs text-muted-foreground">
            <th class="px-3 py-2 font-medium">名称</th>
            <th class="px-3 py-2 font-medium">audience</th>
            <th class="px-3 py-2 font-medium">创建于</th>
            <th class="px-3 py-2 font-medium">过期时间</th>
            <th class="px-3 py-2 font-medium text-right">操作</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="t in tokens" :key="t.id" class="hover:bg-muted/40">
            <td class="px-3 py-2 font-medium">{{ t.name }}</td>
            <td class="px-3 py-2 font-mono text-xs">{{ t.aud }}</td>
            <td class="px-3 py-2 text-xs text-muted-foreground whitespace-nowrap">{{ formatTimestamp(t.created_at) }}</td>
            <td class="px-3 py-2 text-xs whitespace-nowrap" :class="isExpired(t.expires_at) ? 'text-destructive' : 'text-muted-foreground'">
              {{ formatTimestamp(t.expires_at) }}<span v-if="isExpired(t.expires_at)">（已过期）</span>
            </td>
            <td class="px-3 py-2">
              <div class="flex items-center justify-end gap-1">
                <button class="inline-flex items-center rounded-md border border-border p-1.5 hover:bg-muted transition-colors" title="重命名" @click="openRename(t)">
                  <Pencil class="w-3.5 h-3.5" />
                </button>
                <button class="inline-flex items-center rounded-md border border-border p-1.5 hover:bg-muted transition-colors" title="刷新（轮换）" @click="openRefresh(t)">
                  <RefreshCw class="w-3.5 h-3.5" />
                </button>
                <button class="inline-flex items-center rounded-md border border-border p-1.5 text-destructive hover:bg-destructive/10 transition-colors" title="删除（吊销）" @click="deleteTarget = t; deleteError = null">
                  <Trash2 class="w-3.5 h-3.5" />
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <!-- create dialog -->
  <div
    v-if="showCreate"
    v-focus
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 focus:outline-none"
    @click.self="showCreate = false"
    @keydown.esc="showCreate = false"
  >
    <div class="w-full max-w-md rounded-lg border border-border bg-card p-5 space-y-4">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-medium">新建访问令牌</h3>
        <button class="text-muted-foreground hover:text-foreground" @click="showCreate = false"><X class="w-4 h-4" /></button>
      </div>
      <div class="space-y-3">
        <div>
          <label class="block text-xs text-muted-foreground mb-1">名称</label>
          <input
            v-model="newName"
            type="text"
            placeholder="例如：我的笔记本 Claude Code"
            class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            @keyup.enter="submitCreate"
          />
        </div>
        <div>
          <label class="block text-xs text-muted-foreground mb-1">有效期</label>
          <select v-model.number="newTtl" class="rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring">
            <option v-for="o in ttlOptions" :key="o.v" :value="o.v">{{ o.label }}</option>
          </select>
        </div>
        <p class="text-xs text-muted-foreground">
          目标 audience：
          <code v-if="audienceReady" class="font-mono">{{ audience }}</code>
          <span v-else class="text-amber-600">加载中…（无法获取则不可生成）</span>
        </p>
      </div>
      <div v-if="createError" class="rounded-md bg-destructive/10 text-destructive text-xs p-2">{{ createError }}</div>
      <div class="flex justify-end gap-2">
        <button class="rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted transition-colors" @click="showCreate = false">取消</button>
        <button :disabled="creating || !audienceReady" class="inline-flex items-center gap-1.5 rounded-md bg-primary px-4 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity" @click="submitCreate">
          <RefreshCw v-if="creating" class="w-3.5 h-3.5 animate-spin" />
          生成
        </button>
      </div>
    </div>
  </div>

  <!-- token result (shown once) -->
  <div
    v-if="result"
    v-focus
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 focus:outline-none"
    @click.self="closeResult"
    @keydown.esc="closeResult"
  >
    <div class="w-full max-w-lg rounded-lg border border-border bg-card p-5 space-y-3">
      <div class="flex items-center gap-2">
        <ShieldAlert class="w-4 h-4 text-amber-500" />
        <h3 class="text-sm font-medium">令牌已生成 —— 仅此一次完整显示</h3>
      </div>
      <p class="text-xs text-muted-foreground">请立即复制保存。关闭后无法再次查看完整值。</p>
      <div class="flex items-center gap-2">
        <code class="flex-1 rounded-md bg-muted px-3 py-2 text-xs font-mono break-all">{{ maskedResult }}</code>
        <button class="inline-flex items-center rounded-md border border-border px-2.5 py-2 text-xs hover:bg-muted transition-colors" :title="resultRevealed ? '隐藏' : '显示'" @click="resultRevealed = !resultRevealed">
          <component :is="resultRevealed ? EyeOff : Eye" class="w-3.5 h-3.5" />
        </button>
        <button class="inline-flex items-center rounded-md border border-border px-2.5 py-2 text-xs hover:bg-muted transition-colors" @click="copyResult">
          <component :is="copied === 'result' ? Check : Copy" class="w-3.5 h-3.5" />
        </button>
      </div>
      <p v-if="copyFailed" class="text-xs text-destructive">
        复制失败：浏览器剪贴板不可用（需要 HTTPS 或 localhost），请手动选择文本复制
      </p>
      <p class="text-xs text-muted-foreground">过期时间：{{ result.expires_at ? formatTimestamp(result.expires_at) : '—' }}</p>
      <div class="flex justify-end">
        <button class="rounded-md bg-primary px-4 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 transition-opacity" @click="closeResult">完成</button>
      </div>
    </div>
  </div>

  <!-- rename dialog -->
  <div
    v-if="renameTarget"
    v-focus
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 focus:outline-none"
    @click.self="renameTarget = null"
    @keydown.esc="renameTarget = null"
  >
    <div class="w-full max-w-md rounded-lg border border-border bg-card p-5 space-y-4">
      <h3 class="text-sm font-medium">重命名令牌</h3>
      <input
        v-model="renameName"
        type="text"
        class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        @keyup.enter="submitRename"
      />
      <div v-if="renameError" class="rounded-md bg-destructive/10 text-destructive text-xs p-2">{{ renameError }}</div>
      <div class="flex justify-end gap-2">
        <button class="rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted transition-colors" @click="renameTarget = null">取消</button>
        <button :disabled="renaming" class="rounded-md bg-primary px-4 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity" @click="submitRename">保存</button>
      </div>
    </div>
  </div>

  <!-- refresh dialog -->
  <div
    v-if="refreshTarget"
    v-focus
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 focus:outline-none"
    @click.self="refreshTarget = null"
    @keydown.esc="refreshTarget = null"
  >
    <div class="w-full max-w-md rounded-lg border border-border bg-card p-5 space-y-4">
      <h3 class="text-sm font-medium">刷新（轮换）令牌</h3>
      <p class="text-xs text-muted-foreground">
        将为「{{ refreshTarget.name }}」生成一个全新令牌，<span class="text-destructive">旧令牌立即失效</span>。请在客户端替换为新值。
      </p>
      <div>
        <label class="block text-xs text-muted-foreground mb-1">新有效期</label>
        <select v-model.number="refreshTtl" class="rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring">
          <option v-for="o in ttlOptions" :key="o.v" :value="o.v">{{ o.label }}</option>
        </select>
      </div>
      <div v-if="refreshError" class="rounded-md bg-destructive/10 text-destructive text-xs p-2">{{ refreshError }}</div>
      <div class="flex justify-end gap-2">
        <button class="rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted transition-colors" @click="refreshTarget = null">取消</button>
        <button :disabled="refreshing" class="inline-flex items-center gap-1.5 rounded-md bg-primary px-4 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity" @click="submitRefresh">
          <RefreshCw v-if="refreshing" class="w-3.5 h-3.5 animate-spin" />
          轮换
        </button>
      </div>
    </div>
  </div>

  <!-- delete confirm -->
  <div
    v-if="deleteTarget"
    v-focus
    tabindex="-1"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 focus:outline-none"
    @click.self="deleteTarget = null"
    @keydown.esc="deleteTarget = null"
  >
    <div class="w-full max-w-md rounded-lg border border-border bg-card p-5 space-y-4">
      <h3 class="text-sm font-medium">删除令牌</h3>
      <p class="text-xs text-muted-foreground">
        确定删除「{{ deleteTarget.name }}」吗？该令牌将被<span class="text-destructive">吊销</span>，使用它的客户端会在数秒内失去访问权限。此操作不可撤销。
      </p>
      <div v-if="deleteError" class="rounded-md bg-destructive/10 text-destructive text-xs p-2">{{ deleteError }}</div>
      <div class="flex justify-end gap-2">
        <button class="rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted transition-colors" @click="deleteTarget = null">取消</button>
        <button :disabled="deleting" class="inline-flex items-center gap-1.5 rounded-md bg-destructive px-4 py-1.5 text-sm font-medium text-white hover:opacity-90 disabled:opacity-50 transition-opacity" @click="confirmDelete">
          <RefreshCw v-if="deleting" class="w-3.5 h-3.5 animate-spin" />
          删除
        </button>
      </div>
    </div>
  </div>
</template>
