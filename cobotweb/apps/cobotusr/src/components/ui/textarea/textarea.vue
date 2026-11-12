<script setup lang="ts">
import { cn } from '@/lib/utils'
defineOptions({ name: 'UiTextarea', inheritAttrs: false })
withDefaults(
    defineProps<{
        modelValue?: string | null
        class?: string
        invalid?: boolean
        disabled?: boolean
    }>(),
    { invalid: false },
)
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
function update(event: Event) {
    emit('update:modelValue', (event.target as HTMLTextAreaElement).value)
}
</script>
<template>
    <textarea
        v-bind="$attrs"
        :value="modelValue ?? ''"
        :disabled="disabled"
        :aria-invalid="invalid || undefined"
        :class="cn('ui-textarea', invalid && 'has-error', $props.class)"
        @input="update"
    />
</template>
