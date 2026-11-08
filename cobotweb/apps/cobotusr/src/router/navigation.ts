import {
    LayoutDashboard,
    Users,
    ShieldCheck,
    KeyRound,
    AppWindow,
    BadgeCheck,
    ScrollText,
    UserRound,
    Network,
} from '@lucide/vue'
export const navigationGroups = [
    {
        title: '工作空间',
        items: [{ name: 'dashboard-home', title: '工作台', icon: LayoutDashboard }],
    },
    {
        title: '身份管理',
        items: [
            { name: 'user-list', title: '用户管理', icon: Users, children: ['create-user'] },
            { name: 'role-list', title: '角色管理', icon: UserRound },
        ],
    },
    {
        title: '访问控制',
        items: [
            {
                name: 'authority-list',
                title: 'API 权限',
                icon: Network,
                children: ['authority-assign'],
            },
            {
                name: 'permission-list',
                title: '资源权限',
                icon: ShieldCheck,
                children: ['permission-assign'],
            },
        ],
    },
    {
        title: '应用与授权',
        items: [
            { name: 'application-list', title: '应用注册', icon: AppWindow },
            { name: 'authorized-apps-list', title: '已授权应用', icon: KeyRound },
        ],
    },
    { title: '审计', items: [{ name: 'recordings-list', title: '操作记录', icon: ScrollText }] },
]
export const assignmentLinks = [
    { name: 'authority-assign', title: '分配 API 权限', icon: BadgeCheck },
    { name: 'permission-assign', title: '分配资源权限', icon: ShieldCheck },
]
export function activeNavigation(name: unknown) {
    return navigationGroups
        .flatMap((group) => group.items)
        .find(
            (item) =>
                item.name === name || ('children' in item && item.children?.includes(String(name))),
        )
}
