<script setup lang="ts">
import { cn } from '@/lib/utils'
const props = withDefaults(defineProps<{
  modelValue?: boolean
  class?: string
  disabled?: boolean
}>(), {
  modelValue: false,
  disabled: false,
})
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()
function onChange(e: Event) {
  const t = e.target as HTMLInputElement
  emit('update:modelValue', t.checked)
}
const box = 'h-4 w-4 shrink-0 rounded border border-input text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50'
</script>

<template>
  <input type="checkbox" :checked="props.modelValue" :disabled="props.disabled" :class="cn(box, props.class)" @change="onChange" />
</template>