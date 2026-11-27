<script setup lang="ts">
import { Info } from '@lucide/vue'
import { TooltipProvider } from 'reka-ui'
import UiTooltip from '@/components/ui/tooltip/tooltip.vue'

withDefaults(
    defineProps<{
        text: string
        label?: string
        size?: number
    }>(),
    { size: 13 },
)
</script>

<template>
    <TooltipProvider :delay-duration="150">
        <UiTooltip :text="text">
            <button
                type="button"
                class="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-muted-foreground/70 transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60"
                :aria-label="label ?? text"
            >
                <Info :size="size" aria-hidden="true" />
            </button>
        </UiTooltip>
    </TooltipProvider>
</template>
