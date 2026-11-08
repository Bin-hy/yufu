import 'vue-router'

declare module 'vue-router' {
    interface RouteMeta extends Record<PropertyKey, unknown> {
        /** 是否需要认证 */
        requiresAuth?: boolean
        /** 路由标题 */
        title?: string
        /** 是否隐藏在侧边栏 */
        hidden?: boolean
        /** 是否固定在标签栏 */
        affix?: boolean
        /** 路由图标 */
        icon?: string
        workspace?: boolean
        tabIdentity?: 'route' | 'params'
        tabParamKeys?: string[]
        persistQueryKeys?: string[]
        breadcrumb?: string[]
    }
}
