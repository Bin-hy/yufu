<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import AlertDialog from '@/components/ui/alert-dialog/alert-dialog.vue'
import { currentConfirmation, settleConfirmation, cleanupConfirmations, confirmationBusy } from '@/utils/modal'
const busy = confirmationBusy,
    error = ref<string | null>(null)
watch(currentConfirmation, () => {
    busy.value = false
    error.value = null
})
async function confirm() {
    const request = currentConfirmation.value
    if (!request || busy.value) return
    busy.value = true
    try {
        await request.options.confirmCallback?.()
        if (currentConfirmation.value === request) settleConfirmation(true)
    } catch (e) {
        if (currentConfirmation.value === request) error.value = e instanceof Error ? e.message : '操作失败，请重试'
    } finally {
        if (currentConfirmation.value === request) busy.value = false
    }
}
async function cancel() {
    const request = currentConfirmation.value
    if (busy.value) return
    try {
        await request?.options.cancleCallback?.()
    } catch {
        // 取消回调失败仍结束等待。
    } finally {
        if (currentConfirmation.value === request) settleConfirmation(false)
    }
}
onUnmounted(cleanupConfirmations)
</script>
<template>
    <component
        :is="currentConfirmation?.options.destructive ? AlertDialog : UiDialog"
        v-if="currentConfirmation"
        :open="true"
        :focus-origin="currentConfirmation.trigger"
        :title="currentConfirmation.options.title"
        :description="currentConfirmation.options.content"
        :busy="busy"
        :error="error"
        :confirm-text="currentConfirmation.options.confirmText"
        :cancel-text="currentConfirmation.options.cancelText"
        @confirm="confirm"
        @cancel="cancel"
    />
</template>
