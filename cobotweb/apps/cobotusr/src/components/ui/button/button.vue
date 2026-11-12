<script setup lang="ts">
import { LoaderCircle } from '@lucide/vue'
import { cn } from '@/lib/utils'

withDefaults(
    defineProps<{
        variant?: 'default' | 'outline' | 'ghost' | 'secondary' | 'destructive'
        size?: 'sm' | 'default' | 'lg' | 'icon'
        disabled?: boolean
        loading?: boolean
        block?: boolean
        as?: string
        class?: string
        type?: 'button' | 'submit' | 'reset'
    }>(),
    {
        variant: 'default',
        size: 'default',
        as: 'button',
        type: 'button',
        disabled: false,
        loading: false,
        block: false,
    },
)
</script>

<template>
    <component
        :is="as"
        :type="as === 'button' ? type : undefined"
        :disabled="as === 'button' ? disabled || loading : undefined"
        :aria-disabled="as !== 'button' && (disabled || loading) ? true : undefined"
        :aria-busy="loading || undefined"
        :class="
            cn(
                'ui-button',
                'ui-button-' + variant,
                size !== 'default' && 'ui-button-' + size,
                block && 'ui-button-block',
                loading && 'ui-button-loading',
                $props.class,
            )
        "
    >
        <LoaderCircle v-if="loading" class="ui-button-spinner" aria-hidden="true" />
        <slot />
    </component>
</template>
