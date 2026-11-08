import { defineStore } from 'pinia'
import { ref } from 'vue'
export const useLayoutStore = defineStore('layout', () => {
    const sidebarCollapsed = ref(false),
        mobileOpen = ref(false)
    try {
        sidebarCollapsed.value = localStorage.getItem('cobotusr:sidebar') === 'collapsed'
    } catch {
        /* 内存继续 */
    }
    function setSidebarCollapsed(value: boolean) {
        sidebarCollapsed.value = value
        try {
            localStorage.setItem('cobotusr:sidebar', value ? 'collapsed' : 'expanded')
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
