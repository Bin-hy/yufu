<script setup lang="ts">
import {
    DropdownMenuRoot,
    DropdownMenuTrigger,
    DropdownMenuPortal,
    DropdownMenuContent,
} from 'reka-ui'
</script>
<template>
    <DropdownMenuRoot
        ><DropdownMenuTrigger as-child><slot name="trigger" /></DropdownMenuTrigger
        ><DropdownMenuPortal
            ><DropdownMenuContent class="dropdown-content" :side-offset="8" align="end"
                ><slot /></DropdownMenuContent></DropdownMenuPortal
    ></DropdownMenuRoot>
</template>
