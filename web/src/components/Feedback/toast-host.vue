<script setup lang="ts">
import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from '@lucide/vue'
import { storeToRefs } from 'pinia'
import { useToastStore } from '@/pinia/stores/toast'

const toastStore = useToastStore()
const { toasts } = storeToRefs(toastStore)
const icons = {
    success: CircleCheck,
    error: CircleAlert,
    warning: TriangleAlert,
    info: Info,
}
</script>

<template>
    <div class="toast-region">
        <div
            v-for="channel in ['polite', 'assertive'] as const"
            :key="channel"
            :aria-live="channel"
            aria-relevant="additions text"
            class="space-y-2"
        >
            <div
                v-for="t in toasts.filter((t) => (t.type === 'error') === (channel === 'assertive'))"
                :key="t.id"
                :role="t.type === 'error' ? 'alert' : 'status'"
                class="toast-item"
                :class="'is-' + t.type"
            >
                <span class="toast-icon">
                    <component :is="icons[t.type]" :size="15" aria-hidden="true" />
                </span>
                <span class="toast-message">{{ t.message }}</span>
                <button
                    type="button"
                    class="icon-button"
                    aria-label="关闭提示"
                    @click="toastStore.remove(t.id)"
                >
                    <X :size="14" aria-hidden="true" />
                </button>
                <span
                    v-if="t.durationMs && t.durationMs > 0"
                    class="toast-progress"
                    aria-hidden="true"
                    :style="{ animationDuration: t.durationMs + 'ms' }"
                />
            </div>
        </div>
    </div>
</template>
