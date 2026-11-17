<script setup lang="ts">
import { ref, watch } from 'vue'
import YufuMark from './yufu-mark.vue'
const props = defineProps<{ name: string; url?: string | null; collapsed?: boolean; size?: number }>()
const failed = ref(false)
watch(() => props.url, () => { failed.value = false })
</script>
<template>
    <span class="site-brand-logo" :aria-label="name">
        <span class="brand-mark"><img v-if="url && !failed" :src="url" alt="" :width="size ?? 26" :height="size ?? 26" @error="failed = true" /><YufuMark v-else :size="size ?? 26" /></span>
        <span v-if="!collapsed" class="site-brand-name">{{ name }}</span>
    </span>
</template>
