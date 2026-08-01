<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { Copy, Check, ExternalLink, ShieldCheck, KeyRound, Eye, EyeOff, RefreshCw, List, Edit3, Trash2 } from '@lucide/vue'
import { issueMcpToken } from '../../auth'
import { getMcpInfo } from '../../api'
import { mcpEndpoint, maskToken, ttlOptions, useCopy } from './utils'

// ── endpoint configuration ──
const defaultEndpoint = computed(mcpEndpoint)

// Custom endpoint for internal network access (user can override)
const useCustomEndpoint = ref(false)
const customEndpoint = ref('')
const endpoint = computed(() =>
  useCustomEndpoint.value && customEndpoint.value.trim()
    ? customEndpoint.value.trim().replace(/\/$/, '')
    : defaultEndpoint.value
)

// cobotusr 管理端 SPA 地址（权限分配页）。缺省回退到 cobotusr API base。
const cobotusrWeb = computed(() => {
  const v =
    import.meta.env.VITE_COBOTUSR_WEB_URL ??
    import.meta.env.VITE_COBOTSRV_URL ??
    ''
  return (v as string).replace(/\/$/, '')
})
const accessHref = computed(() => `${cobotusrWeb.value}/permissions`)

// ── token generation ──
// Audience must come from the server (== cobotsrv's expected_audience); a token
// minted for the wrong aud is rejected on verification. Generation stays disabled
// until it loads successfully.
const audience = ref('')
const audienceReady = ref(false)
onMounted(async () => {
  try {
    const info = await getMcpInfo()
    audience.value = info.audience || 'cobotrix'
    audienceReady.value = true
  } catch {
    /* leave audience unset; generation stays disabled until a reload succeeds */
  }
})

const ttlDays = ref(30)
const newTokenName = ref('')
const token = ref<string | null>(null)
const expiresAt = ref<string | null>(null)
const generating = ref(false)
const genError = ref<string | null>(null)
const revealed = ref(false)

async function generate() {
  if (!audienceReady.value) return
  generating.value = true
  genError.value = null
  try {
    const res = await issueMcpToken(
      audience.value,
      ttlDays.value,
      newTokenName.value.trim() || undefined,
    )
    token.value = res.access_token
    expiresAt.value = new Date(res.expires_at).toLocaleString()
    revealed.value = false
  } catch (e: unknown) {
    genError.value = e instanceof Error ? e.message : String(e)
  } finally {
    generating.value = false
  }
}

/** Drop the shown token from the page (the server-side record stays; it can be
 * managed on the tokens page). */
function clearToken() {
  token.value = null
  expiresAt.value = null
  revealed.value = false
}

// token shown in snippets: real value only once revealed, masked otherwise —
// never leak the full token into the always-visible curl/JSON snippets
const tokenForSnippet = computed(() => {
  if (!token.value) return '<ACCESS_TOKEN>'
  return revealed.value ? token.value : maskToken(token.value)
})
const maskedToken = computed(() => {
  if (!token.value) return ''
  return revealed.value ? token.value : maskToken(token.value)
})

// ── copy helper ──
const { copied, copyFailed, copy } = useCopy()

// ── command templates (cleaned up) ──

/** Build Claude Code CLI command for MCP registration */
function buildClaudeCodeCmd(ep: string, tok: string): string {
  return `claude mcp add --transport http cobotrix ${ep} --header "Authorization: Bearer ${tok}"`
}

/** Build Claude Desktop mcp-remote config JSON */
function buildMcpRemoteConfig(ep: string, tok: string): object {
  return {
    mcpServers: {
      cobotrix: {
        command: 'npx',
        args: ['-y', 'mcp-remote', ep, '--header', `Authorization: Bearer ${tok}`],
      },
    },
  }
}

/** Build curl test command */
function buildCurlExample(ep: string, tok: string): string {
  const payload = { jsonrpc: '2.0', id: 1, method: 'tools/list', params: {} }
  return `curl -s ${ep} -H "Authorization: Bearer ${tok}" -H 'Content-Type: application/json' -d '${JSON.stringify(payload)}'`
}

const claudeCodeCmd = computed(() => buildClaudeCodeCmd(endpoint.value, tokenForSnippet.value))
const clientConfig = computed(() => JSON.stringify(buildMcpRemoteConfig(endpoint.value, tokenForSnippet.value), null, 2))
const curlExample = computed(() => buildCurlExample(endpoint.value, tokenForSnippet.value))
</script>

<template>
  <div class="space-y-5">
    <!-- access token generator -->
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="flex items-center gap-2 mb-1">
        <KeyRound class="w-4 h-4 text-primary" />
        <h3 class="text-sm font-medium">接入令牌</h3>
      </div>
      <p class="text-xs text-muted-foreground mb-3">
        一键生成长期令牌粘进 MCP 客户端。令牌已持久化、可在「访问令牌」页统一管理（重命名 / 刷新 / 吊销）。
      </p>
      <div class="flex items-end gap-2 flex-wrap">
        <div>
          <label class="block text-xs text-muted-foreground mb-1">名称（可选）</label>
          <input
            v-model="newTokenName"
            type="text"
            placeholder="例如：我的笔记本"
            class="w-40 rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>
        <div>
          <label class="block text-xs text-muted-foreground mb-1">有效期</label>
          <select
            v-model.number="ttlDays"
            class="rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
          >
            <option v-for="o in ttlOptions" :key="o.v" :value="o.v">{{ o.label }}</option>
          </select>
        </div>
        <button
          :disabled="generating || !audienceReady"
          class="inline-flex items-center gap-1.5 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50 transition-opacity"
          @click="generate"
        >
          <RefreshCw v-if="generating" class="w-3.5 h-3.5 animate-spin" />
          <KeyRound v-else class="w-3.5 h-3.5" />
          {{ token ? '重新生成' : '生成令牌' }}
        </button>
        <span class="text-xs text-muted-foreground">
          目标 audience:
          <code v-if="audienceReady" class="font-mono">{{ audience }}</code>
          <span v-else class="text-amber-600">加载中…（无法获取则不可生成）</span>
        </span>
      </div>

      <div v-if="genError" class="mt-3 rounded-md bg-destructive/10 text-destructive text-xs p-2">
        {{ genError }}
      </div>

      <div v-if="token" class="mt-3 space-y-1.5">
        <div class="flex items-center gap-2">
          <code class="flex-1 rounded-md bg-muted px-3 py-2 text-xs font-mono break-all">{{ maskedToken }}</code>
          <button
            class="inline-flex items-center rounded-md border border-border px-2.5 py-2 text-xs hover:bg-muted transition-colors"
            :title="revealed ? '隐藏' : '显示'"
            @click="revealed = !revealed"
          >
            <component :is="revealed ? EyeOff : Eye" class="w-3.5 h-3.5" />
          </button>
          <button
            class="inline-flex items-center gap-1 rounded-md border border-border px-2.5 py-2 text-xs hover:bg-muted transition-colors"
            @click="copy('token', token!)"
          >
            <component :is="copied === 'token' ? Check : Copy" class="w-3.5 h-3.5" />
          </button>
          <button
            class="inline-flex items-center gap-1 rounded-md border border-border px-2.5 py-2 text-xs hover:bg-muted transition-colors"
            title="从页面清除（服务端令牌仍有效）"
            @click="clearToken"
          >
            <Trash2 class="w-3.5 h-3.5" />
          </button>
        </div>
        <p class="text-xs text-muted-foreground">过期时间：{{ expiresAt }} · 仅此一次完整显示，请立即复制保存</p>
      </div>
      <div class="mt-3 pt-3 border-t border-border">
        <RouterLink
          to="/mcp/tokens"
          class="inline-flex items-center gap-1.5 text-xs text-primary hover:underline"
        >
          <List class="w-3.5 h-3.5" />
          管理所有令牌（重命名 / 刷新 / 吊销）
        </RouterLink>
      </div>
    </div>

    <!-- endpoint -->
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-medium">服务端点</h3>
        <button
          class="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1.5 text-xs hover:bg-muted transition-colors"
          @click="useCustomEndpoint = !useCustomEndpoint"
        >
          <Edit3 class="w-3.5 h-3.5" />
          {{ useCustomEndpoint ? '恢复默认' : '自定义' }}
        </button>
      </div>

      <!-- Custom endpoint input for internal network -->
      <div v-if="useCustomEndpoint" class="mb-2">
        <label class="block text-xs text-muted-foreground mb-1">自定义端点（支持内网地址）</label>
        <input
          v-model="customEndpoint"
          type="text"
          placeholder="例如：http://192.168.1.100:8080/mcp"
          class="w-full rounded-md border border-border bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
        />
        <p class="text-xs text-muted-foreground mt-1">
          适用于完全内网化部署。请确保该地址可从客户端访问。
        </p>
      </div>

      <div class="flex items-center gap-2">
        <code class="flex-1 rounded-md bg-muted px-3 py-2 text-xs font-mono break-all">{{ endpoint }}</code>
        <button
          class="inline-flex items-center gap-1 rounded-md border border-border px-2.5 py-2 text-xs hover:bg-muted transition-colors"
          @click="copy('endpoint', endpoint)"
        >
          <component :is="copied === 'endpoint' ? Check : Copy" class="w-3.5 h-3.5" />
        </button>
      </div>
      <p class="text-xs text-muted-foreground mt-2">
        Streamable HTTP（2025-03-26）：<code class="font-mono">POST</code> 收 JSON-RPC，<code class="font-mono">GET</code> 开 SSE。
      </p>
    </div>

    <!-- client config -->
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-medium">Claude Code</h3>
        <button
          class="inline-flex items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-xs hover:bg-muted transition-colors"
          @click="copy('cc', claudeCodeCmd)"
        >
          <component :is="copied === 'cc' ? Check : Copy" class="w-3.5 h-3.5" />
          复制
        </button>
      </div>
      <pre class="rounded-md bg-muted text-xs p-3 overflow-auto whitespace-pre-wrap">{{ claudeCodeCmd }}</pre>
    </div>

    <div class="rounded-lg border border-border bg-card p-4">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-medium">Claude Desktop（mcp-remote 桥接）</h3>
        <button
          class="inline-flex items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-xs hover:bg-muted transition-colors"
          @click="copy('config', clientConfig)"
        >
          <component :is="copied === 'config' ? Check : Copy" class="w-3.5 h-3.5" />
          复制
        </button>
      </div>
      <pre class="rounded-md bg-muted text-xs p-3 overflow-auto whitespace-pre">{{ clientConfig }}</pre>
      <p class="text-xs text-muted-foreground mt-2">
        粘贴到 Claude Desktop 配置文件：<code class="font-mono">~/Library/Application Support/Claude/claude_desktop_config.json</code>
      </p>
    </div>

    <!-- curl -->
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-medium">curl 自测</h3>
        <button
          class="inline-flex items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-xs hover:bg-muted transition-colors"
          @click="copy('curl', curlExample)"
        >
          <component :is="copied === 'curl' ? Check : Copy" class="w-3.5 h-3.5" />
          复制
        </button>
      </div>
      <pre class="rounded-md bg-muted text-xs p-3 overflow-auto whitespace-pre-wrap">{{ curlExample }}</pre>
    </div>

    <!-- access management -->
    <div class="rounded-lg border border-border bg-card p-4 flex items-center justify-between gap-4">
      <div class="flex items-start gap-3">
        <div class="mt-0.5 p-2 rounded-md bg-muted">
          <ShieldCheck class="w-4 h-4 text-foreground" />
        </div>
        <div>
          <h3 class="text-sm font-medium">访问权限管理</h3>
          <p class="text-xs text-muted-foreground mt-0.5">
            在 cobotusr 控制台为角色 / 用户分配 <code class="font-mono">mcp:access</code> 等权限。
          </p>
        </div>
      </div>
      <a
        :href="accessHref"
        target="_blank"
        rel="noopener"
        class="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 transition-opacity shrink-0"
      >
        管理访问权限
        <ExternalLink class="w-3.5 h-3.5" />
      </a>
    </div>

    <!-- clipboard failure notice (non-secure context etc.) -->
    <p
      v-if="copyFailed"
      class="fixed bottom-4 right-4 z-50 rounded-md bg-destructive px-3 py-2 text-xs text-white shadow-lg"
    >
      复制失败：浏览器剪贴板不可用（需要 HTTPS 或 localhost），请手动选择文本复制
    </p>
  </div>
</template>