import 'vue-router'

declare module 'vue-router' {
    interface RouteMeta extends Record<PropertyKey, unknown> {
        /** 是否需要认证 */
        requiresAuth?: boolean,
        /** 路由标题 */
        title?: string
    }
}
