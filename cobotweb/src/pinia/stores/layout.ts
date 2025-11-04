import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useLayoutStore = defineStore('layout', () => {
  const sidebarCollapsed = ref(false)

  const toggleSidebar = () => { sidebarCollapsed.value = !sidebarCollapsed.value }
  const setSidebarCollapsed = (v: boolean) => { sidebarCollapsed.value = !!v }

  return {
    sidebarCollapsed,
    toggleSidebar,
    setSidebarCollapsed,
  }
})