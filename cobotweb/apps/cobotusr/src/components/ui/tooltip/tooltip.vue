<script setup lang="ts">
import { TooltipRoot, TooltipTrigger, TooltipPortal, TooltipContent } from 'reka-ui'
defineProps<{ text: string }>()
</script>
<template>
    <TooltipRoot
        ><TooltipTrigger as-child><slot /></TooltipTrigger
        ><TooltipPortal
            ><TooltipContent class="tooltip-content" side="right" :side-offset="8">{{
                text
            }}</TooltipContent></TooltipPortal
        ></TooltipRoot
    >
</template>
