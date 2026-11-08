<script setup lang="ts">
defineProps<{ title: string; description?: string; eyebrow?: string }>()
</script>
<template>
    <header class="page-header">
        <div>
            <div class="page-eyebrow">{{ eyebrow || 'WORKSPACE' }}</div>
            <h1>{{ title }}</h1>
            <p v-if="description">{{ description }}</p>
        </div>
        <div v-if="$slots.actions || $slots.default" class="page-actions">
            <slot name="actions"><slot /></slot>
        </div>
    </header>
</template>
