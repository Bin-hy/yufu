<script setup lang="ts">
import { ref, onMounted } from 'vue'
import type { ThemeSwitcherProps, Theme } from './theme-switcher.types'

const props = withDefaults(defineProps<ThemeSwitcherProps>(), {
  initialTheme: 'light',
  showLabel: true,
})

const theme = ref<Theme>('light')

function applyTheme(next: Theme) {
  const root = document.documentElement
  if (next === 'dark') {
    root.classList.add('dark')
  } else {
    root.classList.remove('dark')
  }
  localStorage.setItem('theme', next)
  theme.value = next
}

function toggleTheme() {
  applyTheme(theme.value === 'dark' ? 'light' : 'dark')
}

onMounted(() => {
  const persisted = (localStorage.getItem('theme') as Theme | null)
  if (persisted === 'dark' || persisted === 'light') {
    applyTheme(persisted)
    return
  }
  // 系统偏好作为初始值
  const prefersDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches
  applyTheme(persistsOrDefault(prefersDark ? 'dark' : props.initialTheme))
})

function persistsOrDefault(next: Theme): Theme {
  try {
    localStorage.setItem('theme', next)
  } catch (e) {
    // 忽略持久化异常
  }
  return next
}
</script>

<template>
  <button
    type="button"
    :class="[
      'inline-flex items-center justify-center',
      'p-2',
      'rounded-md',
      'text-foreground hover:bg-muted',
      'transition-colors duration-200',
      props.class,
    ]"
    :aria-label="theme === 'dark' ? '切换为浅色' : '切换为暗色'"
    :aria-pressed="theme === 'dark'"
    @click="toggleTheme"
  >
    <!-- 使用内联 SVG，避免额外依赖 -->
    <svg v-if="theme === 'light'" class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="5" />
      <path d="M12 1v2" /><path d="M12 21v2" />
      <path d="M4.22 4.22l1.42 1.42" /><path d="M18.36 18.36l1.42 1.42" />
      <path d="M1 12h2" /><path d="M21 12h2" />
      <path d="M4.22 19.78l1.42-1.42" /><path d="M18.36 5.64l1.42-1.42" />
    </svg>
    <svg v-else class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
    <span v-if="props.showLabel" class="ml-2 text-sm">{{ theme === 'dark' ? '暗色' : '浅色' }}</span>
  </button>

</template>

<!--
  说明：
  - 使用 Tailwind 原子类实现样式，颜色来自 theme.extend 映射的 CSS 变量。
  - 通过切换 html 根节点的 .dark 类与 localStorage 持久化，实现主题切换。
  - 使用内联 SVG 图标以避免额外依赖，适合导航栏图标按钮。
-->