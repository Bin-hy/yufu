<script setup lang="ts">
import { computed, onBeforeUnmount } from 'vue'
import { formatAbsolute, formatRelative, nowSeconds, subscribeClock } from '@/utils/time'
const props = defineProps<{ at: number }>()
const unsubscribe = subscribeClock()
onBeforeUnmount(unsubscribe)
const absolute = computed(() => formatAbsolute(props.at))
const relative = computed(() => formatRelative(props.at, nowSeconds.value))
</script>
<template>
    <time
        :datetime="new Date(at * 1000).toISOString()"
        :title="absolute"
        class="cursor-help underline decoration-dotted decoration-border underline-offset-2"
    >{{ relative }}</time>
</template>
