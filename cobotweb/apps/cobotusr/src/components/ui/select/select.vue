<script setup lang="ts">
import { cn } from '@/lib/utils'
const props = defineProps<{
    modelValue?: string | number | null
    disabled?: boolean
    class?: string
    modelModifiers?: { number?: boolean }
}>()
const emit = defineEmits<{ 'update:modelValue': [value: string | number | null] }>()
function update(event: Event) {
    const select = event.target as HTMLSelectElement
    const option = select.selectedOptions[0] as HTMLOptionElement & { _value?: unknown }
    const raw = option?._value
    if (raw === null || select.value === '' || select.value === 'null') {
        emit('update:modelValue', null)
        return
    }
    const numeric =
        props.modelModifiers?.number ||
        typeof raw === 'number' ||
        typeof props.modelValue === 'number'
    const number = Number(select.value)
    emit('update:modelValue', numeric && Number.isFinite(number) ? number : select.value)
}
</script>
<template>
    <select
        :value="modelValue ?? 'null'"
        :disabled="disabled"
        :class="cn('ui-select', $props.class)"
        @change="update"
    >
        <slot />
    </select>
</template>
