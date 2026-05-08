<script setup lang="ts">
defineProps<{
  modelValue?: string | number
  disabled?: boolean
}>()
defineEmits<{ (e: 'update:modelValue', v: string): void }>()
</script>

<template>
  <select
    :value="modelValue"
    :disabled="disabled"
    class="h-9 rounded-md border border-border bg-transparent px-3 text-sm focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50"
    @change="$emit('update:modelValue', ($event.target as HTMLSelectElement).value)"
  >
    <slot />
  </select>
</template>
