<script setup lang="ts">
withDefaults(
    defineProps<{
        text?: string
        tone?: 'neutral' | 'success' | 'warning' | 'danger' | 'info' | 'brand'
        dot?: boolean
    }>(),
    { tone: 'neutral', dot: false },
)
</script>
<template>
    <span class="status-badge" :class="tone !== 'neutral' && 'status-' + tone">
        <span v-if="dot" class="status-dot" :class="tone === 'warning' && 'status-dot-warning'" aria-hidden="true" />
        {{ text }}<slot />
    </span>
</template>
