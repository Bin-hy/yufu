<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useToastStore } from '@/pinia/stores/toast'

const toastStore = useToastStore()
const { toasts } = storeToRefs(toastStore)
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