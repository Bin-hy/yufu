<script setup lang="ts">
import { TooltipProvider } from 'reka-ui'
import { onMounted, onUnmounted } from 'vue'
import { trackFocusOrigins } from '@/utils/focus'
import ToastHost from '@/components/Feedback/toast-host.vue'
import ConfirmHost from '@/components/Feedback/confirm-host.vue'
let stopTracking: (() => void) | undefined
onMounted(() => {
    stopTracking = trackFocusOrigins()
})
onUnmounted(() => stopTracking?.())
</script>
<template>
    <TooltipProvider :delay-duration="150"><router-view /><ToastHost /><ConfirmHost /></TooltipProvider>
</template>
