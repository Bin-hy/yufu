<script setup lang="ts">
import { Moon, Monitor, Sun } from '@lucide/vue'
import { useTheme } from '@/composables/use-theme'
const { preference, setTheme } = useTheme()
const themes = [
    { value: 'light' as const, title: '浅色', icon: Sun },
    { value: 'dark' as const, title: '暗色', icon: Moon },
    { value: 'system' as const, title: '跟随系统', icon: Monitor },
]
</script>
<template>
    <div class="theme-switcher" role="group" aria-label="主题">
        <button
            v-for="theme in themes"
            :key="theme.value"
            class="icon-button"
            type="button"
            :aria-label="theme.title"
            :aria-pressed="preference === theme.value"
            :title="theme.title"
            @click="setTheme(theme.value)"
        >
            <component :is="theme.icon" :size="14" aria-hidden="true" />
        </button>
    </div>
</template>
