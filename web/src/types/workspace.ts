import type { Ref, ShallowRef } from 'vue'
import type { RouteLocationNormalizedLoaded, RouteLocationRaw } from 'vue-router'
export type TabKey = string
export type SessionEpoch = number
export type CloseScope = 'current' | 'others' | 'right'
export type NavigationOutcome = 'success' | 'unchanged' | 'cancelled' | 'failed'
export interface WorkspaceTab {
    key: TabKey
    routeName: string
    title: string
    fullPath: string
    affix: boolean
    cacheName: string
    scrollTop: number
}
export interface WorkspaceSnapshot {
    version: 1
    ownerId: string
    tabs: Array<Pick<WorkspaceTab, 'routeName' | 'fullPath' | 'scrollTop'>>
    activeKey: TabKey
}
export interface PageContext {
    key: TabKey
    route: Readonly<ShallowRef<RouteLocationNormalizedLoaded>>
    active: Readonly<Ref<boolean>>
    sessionEpoch: Readonly<Ref<SessionEpoch>>
}
export interface WorkspaceActions {
    navigate(to: RouteLocationRaw): Promise<NavigationOutcome>
    close(key: TabKey, scope: CloseScope): Promise<boolean>
    resetSession(): void
}
export interface DraftEntry {
    id: string
    tabKey: TabKey
    title: string
    dirty: () => boolean
    submitting: () => boolean
    discard: () => void
}
export interface PageQuery {
    page_number: number
    page_size: number
    q?: string
    roleId?: number
    application_id?: number
    user_id?: number
}
export interface QueryState<T> {
    data: T | null
    status: 'idle' | 'loading' | 'ready' | 'error'
    refreshing: boolean
    error: string | null
    stale: boolean
}
