import { defineStore } from 'pinia'
import type { RouteLocationNormalized, RouteLocationResolved } from 'vue-router'
import type { WorkspaceTab } from '@/types/workspace'
import { resolveTabIdentity } from '@/workspace/tab-identity'
let serial = 0
export const useTabsStore = defineStore('tabs', {
    state: () => ({
        visited: [] as WorkspaceTab[],
        activeKey: 'dashboard-home',
        invalidations: {} as Record<string, number>,
    }),
    getters: {
        activePath: (state) =>
            state.visited.find((tab) => tab.key === state.activeKey)?.fullPath ?? '/dashboard',
    },
    actions: {
        initHome() {
            if (!this.visited.some((tab) => tab.key === 'dashboard-home'))
                this.visited.unshift({
                    key: 'dashboard-home',
                    routeName: 'dashboard-home',
                    title: '工作台',
                    fullPath: '/dashboard',
                    affix: true,
                    cacheName: 'WorkspacePage_' + ++serial,
                    scrollTop: 0,
                })
        },
        sync(route: RouteLocationNormalized | RouteLocationResolved, activate = true) {
            if (!route.meta.workspace) return
            this.initHome()
            const key = resolveTabIdentity(route)
            const existing = this.visited.find((tab) => tab.key === key)
            if (existing) {
                existing.fullPath = route.fullPath
                existing.title = String(route.meta.title ?? route.name)
            } else
                this.visited.push({
                    key,
                    routeName: String(route.name),
                    title: String(route.meta.title ?? route.name),
                    fullPath: route.fullPath,
                    affix: Boolean(route.meta.affix),
                    cacheName: 'WorkspacePage_' + ++serial,
                    scrollTop: 0,
                })
            if (activate) this.activeKey = key
        },
        commitClose(keys: string[]) {
            this.visited = this.visited.filter((tab) => tab.affix || !keys.includes(tab.key))
        },
        invalidate(routeName: string) {
            this.invalidations[routeName] = (this.invalidations[routeName] ?? 0) + 1
        },
        reset() {
            this.visited = []
            this.activeKey = 'dashboard-home'
            this.invalidations = {}
            this.initHome()
        },
    },
})
