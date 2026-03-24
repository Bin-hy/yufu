<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { useRouter } from 'vue-router';
import Item from './item.vue';
import { useUserStore } from '@/pinia/stores/user'
import UiButton from '@/components/ui/button/button.vue'
import ThemeSwitcher from '@/components/ui/theme-switcher/theme-switcher.vue'
import { logoutAndRedirect } from '@/common/utils/auth'
import { useLayoutStore } from '@/pinia/stores/layout'

// 与 item.vue 保持一致的类型定义，避免模板中出现 unknown
interface NavMeta {
  title?: string
  icon?: any
  hidden?: boolean
}
interface NavItem {
  path: string
  meta?: NavMeta
  children?: NavItem[]
}

const props = withDefaults(defineProps<{
  type?: 'horizontal' | 'vertical'
  items?: NavItem[]
  activeItem?: string
  collapsed?: boolean
}>(), {
  type: 'horizontal',
  items: (_props) => [] as NavItem[],
  activeItem: '',
  collapsed: false
})

const router = useRouter();
const userStore = useUserStore()
const nickname = computed(() => userStore.userInfo?.nickname || userStore.userInfo?.username || '个人中心')

const navClasses = computed(() => ({
  'bg-background': true,
  'border-b': true,
  'border-border': true,
  'shadow-sm': true,
  'min-h-14': props.type === 'horizontal',
  'transition-all': true,
  'duration-300': true,
  'flex': true,
  'items-center': props.type === 'horizontal',
  'flex-row': props.type === 'horizontal',
  'flex-col': props.type === 'vertical'
}));

const projectName = computed(() => import.meta.env.VITE_APP_TITLE || 'Cobotrix')

const layout = useLayoutStore()

// 头像下拉
const menuOpen = ref(false)
const toggleMenu = () => { menuOpen.value = !menuOpen.value }
const closeMenu = () => { menuOpen.value = false }

const onDocClick = (e: MouseEvent) => {
  const target = e.target as HTMLElement
  // 点击在菜单内部不关闭；否则关闭
  if (!target.closest?.('#user-menu-root')) {
    closeMenu()
  }
}

onMounted(() => {
  document.addEventListener('click', onDocClick)
})

onBeforeUnmount(() => {
  document.removeEventListener('click', onDocClick)
})

const handleLogout = () => {
  logoutAndRedirect(router)
}
</script>

<template>
  <nav :class="navClasses">
    <!-- 左侧：折叠按钮 + Logo -->
    <div class="flex items-center h-full px-3 gap-2">
      <UiButton variant="ghost" size="sm" aria-label="切换侧栏" @click="layout.toggleSidebar()">
        <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </UiButton>
      <slot name="logo">
        <div class="flex items-center">
          <svg class="h-6 w-6 text-primary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          <span v-if="!collapsed" class="ml-2 text-base font-semibold">{{ projectName }}</span>
        </div>
      </slot>
    </div>

    <!-- 中间：导航项 -->
    <div class="flex items-center h-full">
      <slot name="items">
        <div class="flex h-full">
          <template v-for="(item, _index) in items" :key="item.path || _index">
            <Item :item="item" :base-path="item.path" />
          </template>
        </div>
      </slot>
    </div>

    <!-- 右侧：操作区（主题切换 + 个人中心下拉） -->
    <div class="ml-auto flex items-center h-full px-3 gap-2">
      <slot name="actions">
        <!-- 主题切换图标按钮 -->
        <ThemeSwitcher :show-label="false" />
        <div id="user-menu-root" class="relative">
          <button type="button" class="flex items-center gap-2" @click="toggleMenu">
            <div class="rounded-full bg-primary text-primary-foreground w-6 h-6 flex items-center justify-center">
              <span class="text-xs">{{ (nickname || '访客')[0] }}</span>
            </div>
            <UiButton variant="ghost" size="sm" class="hidden sm:inline truncate max-w-[8rem]">{{ nickname }}</UiButton>
            <svg class="h-4 w-4 text-muted-foreground hidden sm:block" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
            </svg>
          </button>
          <div v-if="menuOpen" class="absolute right-0 top-full mt-2 w-40 rounded-md border border-border bg-popover text-popover-foreground shadow-lg">
            <router-link to="/profile" class="block px-3 py-2 hover:bg-muted transition-colors" @click="closeMenu">个人中心</router-link>
            <button class="block w-full text-left px-3 py-2 hover:bg-muted transition-colors" @click="handleLogout">退出登录</button>
          </div>
        </div>
      </slot>
    </div>
  </nav>
</template>

<style scoped>
.title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
