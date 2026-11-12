<script setup lang="ts">
import {
    AlertDialogRoot,
    AlertDialogPortal,
    AlertDialogOverlay,
    AlertDialogContent,
    AlertDialogTitle,
    AlertDialogDescription,
} from 'reka-ui'
import { TriangleAlert } from '@lucide/vue'
import UiButton from '@/components/ui/button/button.vue'
import { useId, watch, nextTick } from 'vue'
import { captureFocusOrigin, returnFocus } from '@/utils/focus'
const props = withDefaults(
    defineProps<{
        open: boolean
        title: string
        description?: string
        busy?: boolean
        error?: string | null
        confirmText?: string
        cancelText?: string
        focusOrigin?: HTMLElement | null
    }>(),
    { confirmText: '确认', cancelText: '取消', busy: false },
)
const emit = defineEmits<{ 'update:open': [value: boolean]; confirm: []; cancel: [] }>()
function cancel() {
    if (!props.busy) {
        emit('cancel')
        emit('update:open', false)
    }
}
const cancelId = useId()
let trigger: HTMLElement | null = null
watch(
    () => props.open,
    (open) => {
        if (open) trigger = props.focusOrigin ?? captureFocusOrigin()
    },
    { immediate: true, flush: 'sync' },
)
function openFocus(event: Event) {
    event.preventDefault()
    void nextTick(() => document.getElementById(cancelId)?.focus())
}
function restoreFocus(event: Event) {
    event.preventDefault()
    void nextTick(() => {
        returnFocus(trigger)
    })
}
</script>
<template>
    <AlertDialogRoot
        :open="open"
        @update:open="
            (value) => {
                if (!value) cancel()
            }
        "
    >
        <AlertDialogPortal>
            <AlertDialogOverlay class="dialog-overlay" />
            <AlertDialogContent
                class="dialog-content dialog-md"
                :aria-busy="busy"
                @escape-key-down="
                    (event) => {
                        if (busy) event.preventDefault()
                    }
                "
                @open-auto-focus="openFocus"
                @close-auto-focus="restoreFocus"
            >
                <header class="dialog-header !border-b-0 !pb-0">
                    <div class="flex items-start gap-3">
                        <span
                            class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-destructive/10 text-destructive"
                        >
                            <TriangleAlert :size="17" aria-hidden="true" />
                        </span>
                        <div>
                            <AlertDialogTitle class="dialog-title">{{ title }}</AlertDialogTitle>
                            <AlertDialogDescription class="dialog-description">{{
                                description || '此操作会改变当前对象，请确认后继续。'
                            }}</AlertDialogDescription>
                        </div>
                    </div>
                </header>
                <div class="dialog-body">
                    <slot />
                    <p v-if="error" role="alert" class="inline-error mt-4">{{ error }}</p>
                </div>
                <footer class="dialog-footer">
                    <UiButton :id="cancelId" variant="outline" :disabled="busy" @click="cancel">{{
                        cancelText
                    }}</UiButton>
                    <UiButton
                        variant="destructive"
                        :disabled="busy"
                        :loading="busy"
                        @click="emit('confirm')"
                        >{{ confirmText }}</UiButton
                    >
                </footer>
            </AlertDialogContent>
        </AlertDialogPortal>
    </AlertDialogRoot>
</template>
