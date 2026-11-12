import type { RouteLocationNormalized, RouteLocationResolved } from 'vue-router'
export function resolveTabIdentity(route: RouteLocationNormalized | RouteLocationResolved): string {
    if (!route.name || !route.meta.workspace) return ''
    const name = String(route.name)
    if (route.meta.tabIdentity !== 'params') return name
    const params = (route.meta.tabParamKeys ?? []).map((key) => {
        const value = route.params[key]
        if (Array.isArray(value) || value == null || value === '')
            throw new Error('页面对象标识无效')
        return key + '=' + encodeURIComponent(String(value))
    })
    return name + ':' + params.join('&')
}
