<script setup>
import { computed } from 'vue';
import Link from './link.vue';

const props = defineProps({
  item: {
    type: Object,
    required: true
  },
  basePath: {
    type: String,
    default: ''
  }
});

// 判断是否为外部链接
const isExternalLink = (path) => {
  return /^(https?:|mailto:|tel:)/.test(path);
};

// 解析路径
function resolvePath(routePath) {
  if (isExternalLink(routePath)) {
    return routePath;
  }
  if (isExternalLink(props.basePath)) {
    return props.basePath;
  }
  return props.basePath + '/' + routePath.replace(/^\//, '');
}

// 显示的子菜单
const showingChildren = computed(() => {
  return props.item.children?.filter(child => !child.meta?.hidden) || [];
});

// 显示的子菜单数量
const showingChildNumber = computed(() => showingChildren.value.length);

// 唯一的子菜单项
const theOnlyOneChild = computed(() => {
  const number = showingChildNumber.value;
  if (number > 1) {
    return null;
  } else if (number === 1) {
    return showingChildren.value[0];
  } else {
    return { ...props.item, path: '' };
  }
});
</script>

<template>
  <template v-if="theOnlyOneChild && !theOnlyOneChild.children">
    <Link v-if="theOnlyOneChild.meta" :to="resolvePath(theOnlyOneChild.path)">
      <div class="px-4 py-2 flex items-center cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors">
        <span v-if="theOnlyOneChild.meta.icon" class="mr-2">
          <component :is="theOnlyOneChild.meta.icon" class="h-5 w-5" />
        </span>
        <span class="title">{{ theOnlyOneChild.meta.title }}</span>
      </div>
    </Link>
  </template>
  <div v-else class="submenu">
    <div class="submenu-title px-4 py-2 flex items-center cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700">
      <span v-if="item.meta?.icon" class="mr-2">
        <component :is="item.meta.icon" class="h-5 w-5" />
      </span>
      <span v-if="item.meta?.title" class="title">{{ item.meta.title }}</span>
    </div>
    <div v-if="item.children" class="pl-4">
      <item
        v-for="child in showingChildren"
        :key="child.path"
        :item="child"
        :base-path="resolvePath(child.path)"
      />
    </div>
  </div>
</template>

<style scoped>
.title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
