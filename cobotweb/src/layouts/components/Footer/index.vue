<script lang="ts" setup>
import { computed } from 'vue';

// Footer组件，用于显示页脚信息
// 获取项目名称
const projectName = computed(() => {
  return import.meta.env.VITE_APP_TITLE || 'Cobotrix'
})
</script>

<template>
  <footer class="py-5 px-6 border-t border-border bg-muted/30 text-muted-foreground text-sm">
    <div class="container mx-auto">
      <div class="flex flex-col md:flex-row justify-between items-center">
        <div class="flex items-center mb-4 md:mb-0">
          <div class="mr-3">
            <span class="text-xl font-bold text-primary">{{ projectName }}</span>
          </div>
          <span>© {{ new Date().getFullYear() }} 保留所有权利</span>
        </div>
        <div class="flex space-x-6">
          <a href="#" class="flex items-center hover:text-primary transition-colors">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
            <span>隐私政策</span>
          </a>
          <a href="#" class="flex items-center hover:text-primary transition-colors">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            <span>使用条款</span>
          </a>
          <a href="#" class="flex items-center hover:text-primary transition-colors">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 mr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <span>帮助中心</span>
          </a>
        </div>
      </div>
    </div>
  </footer>
</template>
