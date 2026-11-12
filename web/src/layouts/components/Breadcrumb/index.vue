<script setup lang="ts">
import { computed } from 'vue'
import { ChevronRight } from '@lucide/vue'
import { useRoute } from 'vue-router'
import { activeNavigation } from '@/router/navigation'
const route = useRoute()
const parent = computed(() => activeNavigation(route.name))
const title = computed(() => String(route.meta.title ?? parent.value?.title ?? '工作台'))
</script>
<template>
    <nav class="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground" aria-label="面包屑">
        <router-link to="/dashboard" class="hidden transition-colors hover:text-foreground sm:block"
            >工作空间</router-link
        >
        <ChevronRight v-if="parent" :size="11" class="hidden shrink-0 opacity-45 sm:block" aria-hidden="true" />
        <router-link
            v-if="parent && parent.name !== route.name"
            :to="{ name: parent.name }"
            class="hidden transition-colors hover:text-foreground md:block"
            >{{ parent.title }}</router-link
        >
        <ChevronRight
            v-if="parent && parent.name !== route.name"
            :size="11"
            class="hidden shrink-0 opacity-45 md:block"
            aria-hidden="true"
        />
        <span class="truncate font-medium text-foreground" aria-current="page">{{ title }}</span>
    </nav>
</template>
