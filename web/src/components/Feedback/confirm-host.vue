<script setup lang="ts">
import { shallowRef, watch, onUnmounted } from 'vue'
import { normalizeFailure, type BusinessFailure } from '@/http/errors'
import FailureFeedback from './failure-feedback.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import AlertDialog from '@/components/ui/alert-dialog/alert-dialog.vue'
import { currentConfirmation, settleConfirmation, cleanupConfirmations, confirmationBusy } from '@/utils/modal'
const busy = confirmationBusy,
    failure = shallowRef<BusinessFailure | null>(null)
watch(currentConfirmation, () => {
    busy.value = false
    failure.value = null
})
async function confirm() {
    const request = currentConfirmation.value
    if (!request || busy.value || failure.value?.outcomeUncertain) return
    busy.value = true
    try {
        await request.options.confirmCallback?.()
        if (currentConfirmation.value === request) settleConfirmation(true)
    } catch (e) {
        if (currentConfirmation.value === request) {
            const config = (e as { config?: { method?: string } } | null)?.config
            const result = normalizeFailure(e, config?.method ? {} : { operation: 'write' })
            if (!result.cancelled) failure.value = result
        }
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
        :confirm-disabled="failure?.outcomeUncertain"
        :confirm-text="currentConfirmation.options.confirmText"
        :cancel-text="currentConfirmation.options.cancelText"
        @confirm="confirm"
        @cancel="cancel"
    >
        <FailureFeedback :failure="failure" />
    </component>
</template>
