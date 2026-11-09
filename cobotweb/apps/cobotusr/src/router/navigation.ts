import { LayoutDashboard, Users, AppWindow, KeyRound, ScrollText, UserRound, Network, ShieldCheck } from '@lucide/vue'
import type { Component } from 'vue'
import type { CenterSession } from '@/pages/account/api/type'
export interface NavigationItem { name: string; title: string; icon: Component; children?: string[] }
export interface NavigationGroup { title: string; qualification?: 'developer' | 'administrator'; items: NavigationItem[] }
export const navigationGroups: NavigationGroup[] = [
    { title: '账户中心', items: [
        { name: 'dashboard-home', title: '工作台', icon: LayoutDashboard },
        { name: 'account-profile', title: '账户资料', icon: UserRound },
        { name: 'account-apps', title: '可访问应用', icon: AppWindow },
        { name: 'account-consents', title: '资料授权', icon: KeyRound },
        { name: 'account-sessions', title: '登录会话', icon: ShieldCheck },
    ] },
    { title: '开发者控制台', qualification: 'developer', items: [
        { name: 'developer-apps', title: '我的应用', icon: AppWindow },
    ] },
    { title: '企业管理', qualification: 'administrator', items: [
        { name: 'admin-accounts', title: '账户与邀请', icon: Users },
        { name: 'admin-groups', title: '用户组', icon: Network },
        { name: 'admin-apps', title: '应用与审批', icon: AppWindow },
        { name: 'admin-migrations', title: '历史接入纳管', icon: ShieldCheck },
        { name: 'admin-audit', title: '身份审计', icon: ScrollText },
        { name: 'admin-deliveries', title: '退出通知', icon: Network },
    ] },
]
export function navigationFor(identity: Pick<CenterSession, 'developer_enabled' | 'administrator'> | null) {
    if (!identity) return []
    return navigationGroups.filter(group => group.qualification === 'developer' ? identity.developer_enabled
        : group.qualification === 'administrator' ? identity.administrator : true)
}
export const assignmentLinks: NavigationItem[] = []
export function activeNavigation(name: unknown) {
    return navigationGroups.flatMap(group => group.items).find(item => item.name === name || item.children?.includes(String(name)))
}
