<script setup lang="ts">
import { CheckCircle2, XCircle } from '@lucide/vue'
import type { FeedbackEntry } from '../../composables/useControlEngine'

defineProps<{ log: FeedbackEntry[] }>()

function formatTime(d: Date): string {
  return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}
</script>

<template>
  <div class="rounded-lg border border-border bg-card p-3">
    <h4 class="text-xs font-medium text-muted-foreground mb-2">操作日志</h4>
    <div v-if="log.length === 0" class="text-xs text-muted-foreground">暂无操作</div>
    <ul v-else class="space-y-0.5 max-h-48 overflow-auto">
      <li
        v-for="(entry, i) in log"
        :key="i"
        class="flex items-center gap-2 text-xs px-1.5 py-1 rounded hover:bg-muted/40"
      >
        <component
          :is="entry.success ? CheckCircle2 : XCircle"
          class="w-3 h-3 shrink-0"
          :class="entry.success ? 'text-green-500' : 'text-destructive'"
        />
        <span class="font-mono flex-1 truncate">{{ entry.key }}</span>
        <span class="text-muted-foreground">{{ entry.durationMs }}ms</span>
        <span class="text-muted-foreground">{{ formatTime(entry.timestamp) }}</span>
      </li>
    </ul>
  </div>
</template>
