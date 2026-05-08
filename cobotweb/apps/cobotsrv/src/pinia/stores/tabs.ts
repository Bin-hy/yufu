import { defineStore } from 'pinia'
import type { RouteLocationNormalized } from 'vue-router'

export interface TabItem {
  title: string
  path: string
  affix?: boolean
}

export const useTabsStore = defineStore('tabs', {
  state: () => ({
    visited: [] as TabItem[],
    activePath: '' as string
  }),
  actions: {
    initHome(path = '/dashboard', title = '仪表盘首页') {
      const exist = this.visited.find(v => v.path === path)
      if (!exist) {
        this.visited.unshift({ path, title, affix: true })
      }
      this.activePath = path
    },
    add(route: RouteLocationNormalized) {
      const title = String(route.meta?.title ?? route.name ?? route.path)
      const affix = Boolean(route.meta?.affix)
      const item: TabItem = { title, path: route.path, affix }
      const exist = this.visited.find(v => v.path === item.path)
      if (!exist) this.visited.push(item)
      this.activePath = item.path
    },
    remove(path: string) {
      const idx = this.visited.findIndex(v => v.path === path)
      if (idx >= 0) this.visited.splice(idx, 1)
      if (this.activePath === path) {
        const next = this.visited[idx - 1] || this.visited[0]
        this.activePath = next ? next.path : ''
      }
    },
    closeOthers(path: string) {
      this.visited = this.visited.filter(v => v.affix || v.path === path)
      this.activePath = path
    },
    setActive(path: string) {
      this.activePath = path
    }
  }
})