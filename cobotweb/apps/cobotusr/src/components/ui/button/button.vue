<script setup lang="ts">
import { cn } from '@/lib/utils'
withDefaults(
    defineProps<{
        variant?: 'default' | 'outline' | 'ghost' | 'secondary' | 'destructive'
        size?: 'sm' | 'default' | 'lg' | 'icon'
        disabled?: boolean
        as?: string
        class?: string
        type?: 'button' | 'submit' | 'reset'
    }>(),
    { variant: 'default', size: 'default', as: 'button', type: 'button' },
)
</script>
<template>
    <component
        :is="as"
        :type="as === 'button' ? type : undefined"
        :disabled="as === 'button' ? disabled : undefined"
        :aria-disabled="as !== 'button' && disabled ? true : undefined"
        :class="
            cn(
                'ui-button',
                'ui-button-' + variant,
                size !== 'default' && 'ui-button-' + size,
                $props.class,
            )
        "
        ><slot
    /></component>
</template>
