<script setup lang="ts">
import InfoHint from './info-hint.vue'

withDefaults(
    defineProps<{
        title: string
        description?: string
        eyebrow?: string
        hint?: string
    }>(),
    { eyebrow: '工作区' },
)
</script>
<template>
    <header class="page-header">
        <div class="page-heading">
            <div class="page-eyebrow">{{ eyebrow }}</div>
            <div class="flex items-center gap-2">
                <h1 class="page-title">{{ title }}</h1>
                <InfoHint v-if="hint" :text="hint" :size="14" />
            </div>
            <p v-if="description" class="page-description">{{ description }}</p>
            <div v-if="$slots.meta" class="page-meta"><slot name="meta" /></div>
        </div>
        <div v-if="$slots.actions || $slots.default" class="page-actions">
            <slot name="actions"><slot /></slot>
        </div>
    </header>
</template>
