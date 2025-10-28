<script setup lang="ts">
import { cn } from '@/lib/utils'

const props = withDefaults(defineProps<{
  modelValue?: string | number
  disabled?: boolean
  class?: string
}>(), {
  disabled: false,
})

const emit = defineEmits<{ (e: 'update:modelValue', v: string | number): void }>()

function onChange(e: Event) {
  const t = e.target as HTMLSelectElement
  const value = t.value
  emit('update:modelValue', value)
}

// 基础样式参照 shadcn 输入控件
const base = 'flex h-9 w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50'
</script>

<template>
  <select :value="props.modelValue as any" :disabled="props.disabled" :class="cn(base, props.class)" @change="onChange">
    <slot />
  </select>
</template>