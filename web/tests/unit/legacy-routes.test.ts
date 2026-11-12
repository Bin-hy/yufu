import { describe, expect, it } from 'vitest'
import { constantRoutes, workspaceRoutes } from '@/router/routes'

const all = [...workspaceRoutes, ...constantRoutes]
const route = (path: string) => all.find((entry) => entry.path === path)

describe('旧 RBAC 管理面已退役', () => {
    it('旧路径重定向到新控制台的对应页面', () => {
        expect(route('/users')?.redirect).toBe('/admin/accounts')
        expect(route('/users/create')?.redirect).toBe('/admin/accounts')
        expect(route('/roles')?.redirect).toBe('/admin/groups')
        expect(route('/authority')?.redirect).toBe('/admin/apps')
        expect(route('/authority/assign')?.redirect).toBe('/admin/apps')
        expect(route('/permissions')?.redirect).toBe('/admin/apps')
        expect(route('/permissions/assign')?.redirect).toBe('/admin/apps')
        expect(route('/recordings')?.redirect).toBe('/admin/audit')
        expect(route('/applications')?.redirect).toBe('/developer/apps')
        expect(route('/authorized-apps')?.redirect).toBe('/account/consents')
    })

    it('旧路径不再挂载页面组件', () => {
        for (const path of ['/users', '/roles', '/authority', '/permissions', '/recordings'])
            expect(route(path)?.component, path).toBeUndefined()
    })

    it('/legacy 别名直接落到新入口', () => {
        expect(route('/legacy/users')?.redirect).toBe('/admin/accounts')
        expect(route('/legacy/recordings')?.redirect).toBe('/admin/audit')
    })
})

