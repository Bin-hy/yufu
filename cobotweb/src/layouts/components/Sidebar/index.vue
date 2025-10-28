<template>
  <aside
    class="h-full bg-muted dark:bg-neutral-900 border-r border-border dark:border-neutral-800 overflow-y-auto transition-all duration-300 ease-in-out"
    :class="collapsed ? 'w-16' : 'w-64'"
  >
    <!-- 内容区 -->
    <nav class="flex flex-col p-3 space-y-1 font-inter text-sm">
      <!-- 标题 -->
      <div v-if="!collapsed" class="px-3 pt-2 pb-1 text-xs font-semibold uppercase text-muted-foreground tracking-wider">
        导航
      </div>

      <!-- 首页 -->
      <router-link
        v-if="!hasDashboard"
        to="/dashboard"
        class="flex items-center gap-3 rounded-md px-3 py-2 transition-all duration-150"
        :class="[
          isActivePath('/dashboard')
            ? 'bg-primary/10 text-primary font-semibold'
            : 'text-muted-foreground hover:bg-accent'
        ]"
      >
        <span v-if="collapsed" class="font-bold text-lg">🏠</span>
        <span v-else>首页</span>
      </router-link>

      <!-- 菜单循环 -->
      <div v-for="m in menus" :key="m.key">
        <!-- 有子菜单 -->
        <div v-if="m.children && m.children.length">
          <UiButton
            variant="ghost"
            class="flex w-full items-center justify-between rounded-md px-3 py-2 text-left font-medium transition-colors duration-150"
            :class="[
              isParentOpen(m)
                ? 'bg-muted text-primary'
                : 'text-muted-foreground hover:bg-accent'
            ]"
            @click="toggleParent(m.key)"
          >
            <span class="truncate">
              <span v-if="collapsed" class="font-bold text-lg">{{ getIconText(m) }}</span>
              <span v-else>{{ m.title }}</span>
            </span>
            <svg
              v-if="!collapsed"
              class="h-4 w-4 transform transition-transform duration-200"
              :class="{ 'rotate-90': isParentOpen(m) }"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
          </UiButton>

          <!-- 子菜单 -->
          <transition name="fade">
            <ul
              v-show="isParentOpen(m)"
              class="ml-3 border-l border-border mt-1 space-y-1 pl-3"
            >
              <li v-for="c in m.children" :key="c.key">
                <router-link
                  :to="c.path"
                  class="block rounded-md px-3 py-1.5 text-sm transition-all duration-150"
                  :class="[
                    isActivePath(c.path)
                      ? 'text-primary bg-primary/10 font-medium'
                      : 'text-muted-foreground hover:text-primary hover:bg-accent'
                  ]"
                >
                  <span v-if="collapsed">{{ getIconText(c) }}</span>
                  <span v-else>{{ c.title }}</span>
                </router-link>
              </li>
            </ul>
          </transition>
        </div>

        <!-- 无子菜单 -->
        <router-link
          v-else
          :to="m.path"
          class="flex items-center rounded-md px-3 py-2 transition-all duration-150"
          :class="[
            isActivePath(m.path)
              ? 'bg-primary/10 text-primary font-semibold'
              : 'text-muted-foreground hover:bg-accent'
          ]"
        >
          <span v-if="collapsed" class="font-bold text-lg">{{ getIconText(m) }}</span>
          <span v-else>{{ m.title }}</span>
        </router-link>
      </div>
    </nav>
  </aside>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useMenu, type MenuItem } from '@/composables/use-menu.ts'
import UiButton from '@/components/ui/button/button.vue'

const props = defineProps<{ collapsed?: boolean }>()
const collapsed = computed(() => props.collapsed ?? false)

const route = useRoute()
const { menus } = useMenu('/')

const isActivePath = (path: string) => route.path === path || route.path.startsWith(path + '/')

const hasDashboard = computed(() =>
  (menus.value || []).some(m => m.path === '/dashboard' || (m.children || []).some(c => c.path === '/dashboard'))
)

const getIconText = (m: MenuItem) => (m.title?.trim()?.[0]?.toUpperCase() ?? '*')

const openKeys = ref<Set<string>>(new Set())
const isParentOpen = (m: MenuItem) => !collapsed.value && openKeys.value.has(m.key)
const toggleParent = (key: string) => {
  const s = new Set(openKeys.value)
  s.has(key) ? s.delete(key) : s.add(key)
  openKeys.value = s
}

const initOpenKeys = () => {
  const s = new Set<string>()
  for (const m of menus.value || []) {
    if ((m.children || []).some(c => isActivePath(c.path)) || isActivePath(m.path)) s.add(m.key)
  }
  openKeys.value = s
}
onMounted(initOpenKeys)
</script>

<style scoped>
aside::-webkit-scrollbar {
  width: 6px;
}
aside::-webkit-scrollbar-thumb {
  background-color: hsl(var(--b3));
  border-radius: 9999px;
}
.fade-enter-active,
.fade-leave-active {
  transition: all 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  transform: translateY(-2px);
}
</style>
