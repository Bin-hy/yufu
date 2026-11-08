<script setup lang="ts">
import { storeToRefs } from 'pinia'
import { useToastStore } from '@/pinia/stores/toast'

const toastStore = useToastStore()
const { toasts } = storeToRefs(toastStore)
</script>

<template>
    <div class="fixed bottom-4 right-4 z-50 space-y-2">
        <div
            v-for="channel in ['polite', 'assertive'] as const"
            :key="channel"
            :aria-live="channel"
            aria-relevant="additions text"
            class="space-y-2"
        >
            <div
                v-for="t in toasts.filter(
                    (t) => (t.type === 'error') === (channel === 'assertive'),
                )"
                :key="t.id"
                :role="t.type === 'error' ? 'alert' : 'status'"
                :class="[
                    'rounded-md border bg-card text-card-foreground shadow-sm px-3 py-2 text-sm flex items-center gap-2',
                    t.type === 'success' && 'border-green-600',
                    t.type === 'error' && 'border-destructive',
                    t.type === 'info' && 'border-blue-600',
                    t.type === 'warning' && 'border-yellow-600',
                ]"
            >
                <span>{{ t.message }}</span>
                <button
                    type="button"
                    class="icon-button"
                    aria-label="关闭提示"
                    @click="toastStore.remove(t.id)"
                >
                    ×
                </button>
            </div>
        </div>
    </div>
</template>
