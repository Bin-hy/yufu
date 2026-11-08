<script setup lang="ts">
import { Boxes } from '@lucide/vue'
defineProps<{ collapsed?: boolean }>()
</script>
<template>
    <router-link
        to="/dashboard"
        class="brand"
        :class="{ '!px-4': collapsed }"
        aria-label="cobotusr 工作台"
        ><span class="brand-mark"><Boxes :size="19" aria-hidden="true" /></span
        ><span v-if="!collapsed">cobot<span class="text-primary">usr</span></span></router-link
    >
</template>
