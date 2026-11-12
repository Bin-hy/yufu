import { defineStore } from 'pinia'
import { ref } from 'vue'
export const useLayoutStore = defineStore('layout', () => {
    const sidebarCollapsed = ref(false),
        mobileOpen = ref(false)
    try {
        sidebarCollapsed.value = localStorage.getItem('yufu:sidebar') === 'collapsed'
    } catch {
        /* 内存继续 */
    }
    function setSidebarCollapsed(value: boolean) {
        sidebarCollapsed.value = value
        try {
            localStorage.setItem('yufu:sidebar', value ? 'collapsed' : 'expanded')
        } catch {
            /* 内存继续 */
        }
    }
    return {
        sidebarCollapsed,
        mobileOpen,
        toggleSidebar: () => setSidebarCollapsed(!sidebarCollapsed.value),
        setSidebarCollapsed,
    }
})
