<script setup>
import { ref, computed } from 'vue';
import { useRoute } from 'vue-router';
import Link from './link.vue';
import Item from './item.vue';
import { useUserStore } from '@/pinia/stores/user'
import UiButton from '@/components/ui/button/button.vue'

const props = defineProps({
  type: { type: String, default: 'horizontal' },
  items: { type: Array, default: () => [] },
  activeItem: { type: String, default: '' },
  collapsed: { type: Boolean, default: false }
});

const route = useRoute();
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
  'flex-row': props.type === 'horizontal',
  'flex-col': props.type === 'vertical'
}));

const emit = defineEmits(['item-click','toggle-sidebar']);
const handleItemClick = (item) => emit('item-click', item);
const projectName = computed(() => import.meta.env.VITE_APP_TITLE || 'Cobotrix')
</script>

<template>
  <nav :class="navClasses">
    <!-- 左侧：折叠按钮 + Logo -->
    <div class="flex items-center h-full px-3 gap-2">
      <UiButton variant="ghost" size="sm" aria-label="切换侧栏" @click="$emit('toggle-sidebar')">
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
          <template v-for="(item, index) in items" :key="index">
            <Item :item="item" :base-path="item.path" />
          </template>
        </div>
      </slot>
    </div>

    <!-- 右侧：个人中心 -->
    <div class="ml-auto flex items-center h-full px-3">
      <slot name="actions">
        <router-link to="/profile" class="flex items-center gap-2">
          <div class="rounded-full bg-primary text-primary-foreground w-6 h-6 flex items-center justify-center">
            <span class="text-xs">{{ (nickname || '访客')[0] }}</span>
          </div>
          <UiButton variant="ghost" size="sm" class="hidden sm:inline truncate max-w-[8rem]">{{ nickname }}</UiButton>
        </router-link>
      </slot>
    </div>
  </nav>
</template>

<style scoped>
.title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
