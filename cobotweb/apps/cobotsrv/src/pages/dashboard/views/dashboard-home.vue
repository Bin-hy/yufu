<script lang="ts" setup>
import { computed } from 'vue'
import { useUserStore } from '@/pinia/stores/user'

const userStore = useUserStore()

// 获取用户名（如果已登录）
const username = computed(() => {
  return userStore.userInfo?.nickname || userStore.userInfo?.username || '管理员'
})
</script>

<template>
  <div>
    <!-- 页面标题 -->
    <div class="mb-6">
      <h1 class="text-2xl font-bold text-gray-800">欢迎回来，{{ username }}</h1>
      <p class="text-gray-600">这是您的管理仪表盘，实时监控系统状态</p>
    </div>

    <!-- 子路由视图：用于显示仪表盘的子页面（如系统介绍） -->
    <router-view />
  </div>
</template>
