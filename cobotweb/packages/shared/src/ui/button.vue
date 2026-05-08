<script setup lang="ts">
import { computed } from 'vue'

type Variant = 'default' | 'outline' | 'ghost' | 'destructive'
type Size = 'sm' | 'md' | 'lg'

const props = withDefaults(
  defineProps<{
    variant?: Variant
    size?: Size
    disabled?: boolean
    type?: 'button' | 'submit' | 'reset'
  }>(),
  { variant: 'default', size: 'md', disabled: false, type: 'button' },
)

defineEmits<{ (e: 'click', ev: MouseEvent): void }>()

const cls = computed(() => {
  const base =
    'inline-flex items-center justify-center rounded-md font-medium transition disabled:opacity-50 disabled:cursor-not-allowed'
  const sizeCls = props.size === 'sm' ? 'h-8 px-3 text-sm' : props.size === 'lg' ? 'h-11 px-6' : 'h-9 px-4 text-sm'
  const variantCls =
    props.variant === 'outline'
      ? 'border border-border bg-transparent hover:bg-muted'
      : props.variant === 'ghost'
        ? 'hover:bg-muted'
        : props.variant === 'destructive'
          ? 'bg-destructive text-destructive-foreground hover:opacity-90'
          : 'bg-primary text-primary-foreground hover:opacity-90'
  return `${base} ${sizeCls} ${variantCls}`
})
</script>

<template>
  <button :type="type" :disabled="disabled" :class="cls" @click="$emit('click', $event)">
    <slot />
  </button>
</template>
