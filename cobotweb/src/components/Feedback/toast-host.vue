<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'

interface ToastItem {
  id: number
  type: 'success' | 'error' | 'info' | 'warning'
  message: string
}

const toasts = ref<ToastItem[]>([])
let uid = 0

function pushToast(type: ToastItem['type'], message: string, durationMs = 3000) {
  const id = ++uid
  toasts.value.push({ id, type, message })
  setTimeout(() => {
    toasts.value = toasts.value.filter(t => t.id !== id)
  }, durationMs)
}

function handleEvent(e: Event) {
  const { type, message, durationMs } = (e as CustomEvent).detail || {}
  pushToast(type || 'info', String(message || ''), durationMs ?? 3000)
}

onMounted(() => {
  window.addEventListener('app:toast', handleEvent as EventListener)
})

onBeforeUnmount(() => {
  window.removeEventListener('app:toast', handleEvent as EventListener)
})
</script>

<template>
  <div class="fixed bottom-4 right-4 z-50 space-y-2">
    <div
      v-for="t in toasts"
      :key="t.id"
      :class="[
        'rounded-md border bg-card text-card-foreground shadow-sm px-3 py-2 text-sm flex items-center gap-2',
        t.type === 'success' && 'border-green-600',
        t.type === 'error' && 'border-destructive',
        t.type === 'info' && 'border-blue-600',
        t.type === 'warning' && 'border-yellow-600'
      ]"
    >
      <span>{{ t.message }}</span>
    </div>
  </div>
</template>