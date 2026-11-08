<script setup lang="ts">
defineProps<{ text?: string }>()
</script>
<template>
    <span class="status-badge">{{ text }}<slot /></span>
</template>
