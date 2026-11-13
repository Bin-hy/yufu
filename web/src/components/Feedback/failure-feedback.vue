<script setup lang="ts">
import { ref } from 'vue'
import { formatFailure, normalizeFailure, type BusinessFailure } from '@/http/errors'
import UiButton from '@/components/ui/button/button.vue'
const props = defineProps<{
    failure?: BusinessFailure | null
    message?: string | null
    recoveryLabel?: string
    recover?: () => unknown | Promise<unknown>
}>()
const busy = ref(false)
const recoveryError = ref('')
async function handleRecovery() {
    if (busy.value || !props.recover) return
    busy.value = true
    recoveryError.value = ''
    try { await props.recover() } catch (error) {
        recoveryError.value = formatFailure(normalizeFailure(error, { operation: 'read' }))
    } finally { busy.value = false }
}
</script>
<template>
    <div v-if="!failure?.cancelled && (failure || message)" class="form-alert mb-4" role="alert">
        <div class="min-w-0 flex-1 space-y-1">
            <p v-if="failure" class="font-medium">{{ failure.title }}</p>
            <p>{{ message || failure?.message }}</p>
            <p v-if="failure?.requestId && !message" class="break-all text-xs">请求标识 {{ failure.requestId }}</p>
            <p v-if="recoveryError">核对失败：{{ recoveryError }}</p>
            <UiButton v-if="recover && recoveryLabel" class="!mt-3" variant="outline" size="sm" :disabled="busy" :loading="busy" @click="handleRecovery">
                {{ recoveryLabel }}
            </UiButton>
        </div>
    </div>
</template>
