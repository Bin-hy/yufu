import { ref } from 'vue'

/** MCP HTTP endpoint shown in copy-paste snippets. `||` (not `??`): an explicitly
 * empty VITE_COBOTRIX_URL means same-origin relative paths (matches http.ts). */
export function mcpEndpoint(): string {
  const base = (import.meta.env.VITE_COBOTRIX_URL || window.location.origin).replace(/\/$/, '')
  return `${base}/mcp`
}

export function formatTimestamp(ts: string): string {
  return new Date(ts).toLocaleString()
}

/** Show only head/tail of a token — used for the display row AND for the
 * copy-paste snippets until the user explicitly reveals the full value. */
export function maskToken(token: string): string {
  return `${token.slice(0, 12)}${'•'.repeat(20)}${token.slice(-6)}`
}

export const ttlOptions = [
  { v: 7, label: '7 天' },
  { v: 30, label: '30 天' },
  { v: 90, label: '90 天' },
]

/** Clipboard copy with per-key "copied" feedback; surfaces failure instead of
 * silently no-op'ing (clipboard API is unavailable on non-secure contexts). */
export function useCopy() {
  const copied = ref<string | null>(null)
  const copyFailed = ref(false)
  let timer: ReturnType<typeof setTimeout> | null = null

  async function copy(key: string, text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text)
      copied.value = key
      copyFailed.value = false
    } catch {
      copied.value = null
      copyFailed.value = true
    }
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => {
      copied.value = null
      copyFailed.value = false
    }, 1500)
  }

  return { copied, copyFailed, copy }
}
