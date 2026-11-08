<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { activeNavigation } from '@/router/navigation'
const route = useRoute()
const parent = computed(() => activeNavigation(route.name))
</script>
<template>
    <nav class="flex min-w-0 items-center gap-2 text-xs text-muted-foreground" aria-label="面包屑">
        <router-link to="/dashboard" class="hidden sm:block">工作空间</router-link
        ><span class="hidden sm:block" aria-hidden="true">/</span
        ><router-link v-if="parent && parent.name !== route.name" :to="{ name: parent.name }">{{
            parent.title
        }}</router-link
        ><span v-if="parent && parent.name !== route.name" aria-hidden="true">/</span
        ><span class="truncate text-foreground" aria-current="page">{{ route.meta.title }}</span>
    </nav>
</template>
