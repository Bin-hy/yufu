<script setup lang="ts">
import { cn } from '@/lib/utils'
const props = withDefaults(
    defineProps<{
        modelValue?: string | number | null
        type?: string
        placeholder?: string
        class?: string
        disabled?: boolean
        modelModifiers?: { number?: boolean; trim?: boolean }
    }>(),
    { type: 'text', placeholder: '' },
)
const emit = defineEmits<{ 'update:modelValue': [value: string | number] }>()
function update(event: Event) {
    const target = event.target as HTMLInputElement
    const value = props.modelModifiers?.trim ? target.value.trim() : target.value
    const number = Number.parseFloat(value)
    emit(
        'update:modelValue',
        props.modelModifiers?.number && !Number.isNaN(number) ? number : value,
    )
}
</script>
<template>
    <input
        :type="type"
        :value="modelValue ?? ''"
        :placeholder="placeholder"
        :disabled="disabled"
        :class="cn('ui-input', $props.class)"
        @input="update"
    />
</template>
