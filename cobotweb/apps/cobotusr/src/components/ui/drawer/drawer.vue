<script setup lang="ts">
import UiDialog from '@/components/ui/dialog/dialog.vue'
defineProps<{ open: boolean; title?: string }>()
defineEmits<{ 'update:open': [value: boolean] }>()
</script>
<template>
    <UiDialog
        :open="open"
        :title="title || '导航'"
        hide-footer
        side
        @update:open="$emit('update:open', $event)"
        ><slot
    /></UiDialog>
</template>
