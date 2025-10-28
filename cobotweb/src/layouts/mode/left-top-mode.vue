<script setup>
import { ref } from 'vue'
import NavigationBar from '@/layouts/components/NavigationBar/index.vue'
import Sidebar from '@/layouts/components/Sidebar/index.vue'
import TagsView from '@/layouts/components/TagsView/index.vue'
import Footer from '@/layouts/components/Footer/index.vue'

const collapsed = ref(false)
</script>

<template>
  <div class="flex flex-col h-screen bg-muted">
    <!-- 头部：精简为单一导航栏 + 标签栏，减少空白 -->
    <header class="sticky top-0 z-20 bg-background/95 backdrop-blur-md border-b border-border shadow-sm">
      <NavigationBar @toggle-sidebar="collapsed = !collapsed" />
      <TagsView />
    </header>

    <!-- 主体：侧栏 + 内容 -->
    <main class="flex flex-1 overflow-hidden">
      <aside class="bg-background border-r border-border shadow-sm" :class="collapsed ? 'w-16' : 'w-64'">
        <Sidebar :collapsed="collapsed" />
      </aside>
      <section class="flex-1 overflow-y-auto p-6 md:p-8">
        <router-view />
      </section>
    </main>
    <!-- 页脚：固定在底部 -->
    <Footer />
  </div>
</template>
