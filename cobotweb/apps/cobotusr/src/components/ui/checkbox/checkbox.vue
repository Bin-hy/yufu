<script setup lang="ts">
import { cn } from '@/lib/utils'
const props = withDefaults(
    defineProps<{
        modelValue?: boolean
        class?: string
        disabled?: boolean
    }>(),
    {
        modelValue: false,
        disabled: false,
    },
)
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()
function onChange(e: Event) {
    const t = e.target as HTMLInputElement
    emit('update:modelValue', t.checked)
}
</script>
<template>
    <input
        type="checkbox"
        :checked="props.modelValue"
        :disabled="props.disabled"
        :class="cn('ui-checkbox', props.class)"
        @change="onChange"
    />
</template>
