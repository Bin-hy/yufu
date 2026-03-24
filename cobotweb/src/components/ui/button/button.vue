<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/lib/utils'

type Variant = 'default' | 'outline' | 'ghost' | 'secondary' | 'destructive'
type Size = 'sm' | 'default' | 'lg' | 'icon'

const props = withDefaults(defineProps<{
  variant?: Variant
  size?: Size
  disabled?: boolean
  as?: string
  class?: string
}>(), {
  variant: 'default',
  size: 'default',
  disabled: false,
  as: 'button',
})

const classes = computed(() => {
  const base = 'inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50 disabled:pointer-events-none'
  const sizes: Record<Size, string> = {
    sm: 'h-8 px-3 py-1',
    default: 'h-9 px-4 py-2',
    lg: 'h-10 px-6 py-2',
    icon: 'h-9 w-9',
  }
  const variants: Record<Variant, string> = {
    default: 'bg-primary text-primary-foreground hover:bg-primary/90',
    outline: 'border border-input bg-background hover:bg-accent hover:text-accent-foreground',
    ghost: 'hover:bg-accent hover:text-accent-foreground',
    secondary: 'bg-secondary text-secondary-foreground hover:bg-secondary/90',
    destructive: 'bg-destructive text-destructive-foreground hover:bg-destructive/90',
  }
  return cn(base, sizes[props.size], variants[props.variant], props.class)
})
</script>

<template>
  <component :is="props.as" :disabled="props.disabled" :class="classes">
    <slot />
  </component>
</template>