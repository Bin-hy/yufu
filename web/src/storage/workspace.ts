import type { Router } from 'vue-router'
import type { WorkspaceSnapshot, WorkspaceTab } from '@/types/workspace'
import { resolveTabIdentity } from '@/workspace/tab-identity'
const prefix = 'yufu:workspace:v1:'
export function workspaceStorageKey(ownerId: string) {
    return prefix + ownerId
}
export function safeWorkspacePath(router: Router, path: string): string | null {
    if (!path.startsWith('/') || path.startsWith('//')) return null
    const route = router.resolve(path)
    if (!route.meta.workspace || !route.name) return null
    const query: Record<string, string> = {}
    for (const key of route.meta.persistQueryKeys ?? []) {
        const value = route.query[key]
        if (typeof value === 'string') query[key] = value
    }
    return router.resolve({ name: route.name, params: route.params, query, hash: '' }).fullPath
}
export function encodeSnapshot(
    router: Router,
    ownerId: string,
    tabs: WorkspaceTab[],
    activeKey: string,
): WorkspaceSnapshot {
    return {
        version: 1,
        ownerId,
        activeKey,
        tabs: tabs.flatMap((tab) => {
            const fullPath = safeWorkspacePath(router, tab.fullPath)
            return fullPath
                ? [
                      {
                          routeName: tab.routeName,
                          fullPath,
                          scrollTop: Math.max(0, Number(tab.scrollTop) || 0),
                      },
                  ]
                : []
        }),
    }
}
export function loadWorkspace(router: Router, ownerId: string | null): WorkspaceSnapshot | null {
    if (!ownerId) return null
    try {
        const raw = localStorage.getItem(workspaceStorageKey(ownerId))
        if (!raw) return null
        const data = JSON.parse(raw)
        if (data.version !== 1 || data.ownerId !== ownerId || !Array.isArray(data.tabs)) return null
        const seen = new Set<string>()
        const tabs = data.tabs.flatMap((tab: { fullPath?: unknown; scrollTop?: unknown }) => {
            if (typeof tab.fullPath !== 'string') return []
            const fullPath = safeWorkspacePath(router, tab.fullPath)
            if (!fullPath) return []
            const route = router.resolve(fullPath)
            const key = resolveTabIdentity(route)
            if (seen.has(key)) return []
            seen.add(key)
            return [
                {
                    routeName: String(route.name),
                    fullPath,
                    scrollTop:
                        typeof tab.scrollTop === 'number' && Number.isFinite(tab.scrollTop)
                            ? Math.max(0, tab.scrollTop)
                            : 0,
                },
            ]
        })
        return {
            version: 1,
            ownerId,
            tabs,
            activeKey: typeof data.activeKey === 'string' ? data.activeKey : 'dashboard-home',
        }
    } catch {
        return null
    }
}
export function saveWorkspace(
    router: Router,
    ownerId: string | null,
    tabs: WorkspaceTab[],
    activeKey: string,
) {
    if (!ownerId) return
    try {
        localStorage.setItem(
            workspaceStorageKey(ownerId),
            JSON.stringify(encodeSnapshot(router, ownerId, tabs, activeKey)),
        )
    } catch {
        /* 导航可在内存继续 */
    }
}
export function clearWorkspace(ownerId: string | null) {
    if (ownerId)
        try {
            localStorage.removeItem(workspaceStorageKey(ownerId))
        } catch {
            /* 存储不可用 */
        }
}
