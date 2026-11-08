<script setup lang="ts">
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle, DialogDescription } from 'reka-ui'
import { X, LoaderCircle } from '@lucide/vue'
import { nextTick, watch, inject, useId } from 'vue'
import { pageContextKey } from '@/workspace/page-context'
import { captureFocusOrigin, returnFocus } from '@/utils/focus'
import UiButton from '@/components/ui/button/button.vue'
const props = withDefaults(
    defineProps<{
        open?: boolean
        title?: string
        description?: string
        size?: 'md' | 'lg' | 'xl'
        contentScrollable?: boolean
        hideFooter?: boolean
        confirmText?: string
        cancelText?: string
        confirmDisabled?: boolean
        closeOnConfirm?: boolean
        busy?: boolean
        error?: string | null
        beforeClose?: () => boolean | Promise<boolean>
        side?: boolean
        focusOrigin?: HTMLElement | null
    }>(),
    {
        open: false,
        title: '确认操作',
        size: 'md',
        contentScrollable: true,
        hideFooter: false,
        confirmText: '确认',
        cancelText: '取消',
        busy: false,
    },
)
const emit = defineEmits<{ 'update:open': [value: boolean]; confirm: []; cancel: [] }>()
let trigger: HTMLElement | null = null
const contentId = useId()
function openFocus(event: Event) {
    const content = document.getElementById(contentId)
    const field = content?.querySelector<HTMLElement>(
        '[autofocus]:not(:disabled), .dialog-body input:not(:disabled), .dialog-body select:not(:disabled), .dialog-body textarea:not(:disabled)',
    )
    if (field) {
        event.preventDefault()
        void nextTick(() => field.focus())
    }
}
const pageContext = inject(pageContextKey, null)
let closing = false
watch(
    () => props.open,
    (open) => {
        if (open) trigger = props.focusOrigin ?? captureFocusOrigin()
    },
    { immediate: true, flush: 'sync' },
)
async function close() {
    if (closing || props.busy) return
    closing = true
    try {
        if (props.beforeClose && !(await props.beforeClose())) return
        emit('cancel')
        emit('update:open', false)
    } finally {
        closing = false
    }
}
function preventBusy(event: Event) {
    if (props.busy) event.preventDefault()
}
function restoreFocus(event: Event) {
    event.preventDefault()
    void nextTick(() => {
        returnFocus(trigger)
    })
}
</script>
<template>
    <DialogRoot
        :open="open && (pageContext?.active.value ?? true)"
        @update:open="
            (value) => {
                if (!value) close()
            }
        "
    >
        <DialogPortal>
            <DialogOverlay class="dialog-overlay" />
            <DialogContent
                :id="contentId"
                class="dialog-content"
                :class="[side ? 'dialog-drawer' : '', 'dialog-' + size]"
                :aria-busy="busy"
                @open-auto-focus="openFocus"
                @escape-key-down="preventBusy"
                @pointer-down-outside="preventBusy"
                @close-auto-focus="restoreFocus"
            >
                <header class="dialog-header">
                    <div>
                        <DialogTitle class="dialog-title">{{ title }}</DialogTitle
                        ><DialogDescription :class="description ? 'dialog-description' : 'sr-only'">{{
                            description || '查看内容后选择确认或取消。'
                        }}</DialogDescription>
                    </div>
                    <button type="button" class="icon-button" aria-label="关闭弹窗" :disabled="busy" @click="close">
                        <X :size="18" aria-hidden="true" />
                    </button>
                </header>
                <div class="dialog-body" :class="{ 'dialog-scroll': contentScrollable }">
                    <slot />
                    <p v-if="error" class="inline-error mt-4" role="alert">{{ error }}</p>
                </div>
                <footer v-if="!hideFooter" class="dialog-footer">
                    <slot name="footer"
                        ><UiButton variant="outline" :disabled="busy" @click="close">{{ cancelText }}</UiButton
                        ><UiButton :disabled="busy || confirmDisabled" @click="emit('confirm')"
                            ><LoaderCircle v-if="busy" class="animate-spin" :size="16" aria-hidden="true" />{{
                                busy ? '处理中…' : confirmText
                            }}</UiButton
                        ></slot
                    >
                </footer>
            </DialogContent>
        </DialogPortal>
    </DialogRoot>
</template>
