<script setup lang="ts">
import NavigationBar from '@/layouts/components/NavigationBar/index.vue'
import Sidebar from '@/layouts/components/Sidebar/index.vue'
import TagsView from '@/layouts/components/TagsView/index.vue'
import AppMain from '@/layouts/components/AppMain/index.vue'
import Drawer from '@/components/ui/drawer/drawer.vue'
import { useLayoutStore } from '@/pinia/stores/layout'
const layout = useLayoutStore()
</script>
<template>
    <a class="skip-link" href="#workspace-content">跳到主内容</a>
    <div class="workspace-shell">
        <aside
            class="workspace-sidebar"
            :class="{ 'is-collapsed': layout.sidebarCollapsed }"
            aria-label="主导航"
        >
            <Sidebar :collapsed="layout.sidebarCollapsed" />
        </aside>
        <div class="workspace-column"><NavigationBar /><TagsView /><AppMain /></div>
    </div>
    <Drawer v-model:open="layout.mobileOpen" title="工作台导航"
        ><Sidebar mobile @navigate="layout.mobileOpen = false"
    /></Drawer>
</template>
