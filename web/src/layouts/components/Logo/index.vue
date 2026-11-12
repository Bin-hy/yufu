<script setup lang="ts">
import YufuMark from '@/components/brand/yufu-mark.vue'
defineProps<{ collapsed?: boolean }>()
</script>
<template>
    <router-link
        to="/dashboard"
        class="brand"
        :class="{ '!justify-center !px-0': collapsed }"
        aria-label="yufu 工作台"
        ><span class="brand-mark"><YufuMark :size="26" /></span
        ><span v-if="!collapsed" class="tracking-[-0.045em]">yu<span class="text-primary">fu</span></span></router-link
    >
</template>
