// composables/useMenu.ts
import { computed } from 'vue'
import { useRouter, useRoute, type RouteRecordRaw, type RouteRecordNormalized } from 'vue-router'

export interface MenuItem {
  key: string
  title: string
  path: string
  icon?: any
  children?: MenuItem[]
}

// 接受 Raw 或 Normalized 两种类型
type AnyRoute = RouteRecordRaw | RouteRecordNormalized

function getChildren(r: AnyRoute): AnyRoute[] {
  // both types use "children" optional
  return (r.children as AnyRoute[] | undefined) ?? []
}

function getMeta(r: AnyRoute) {
  // meta may be undefined
  return (r as any).meta ?? {}
}

function getName(r: AnyRoute) {
  return (r as any).name ?? undefined
}

/**
 * 从 router 或静态路由构建菜单树（兼容 Raw & Normalized）
 */
export function useMenu(rootPath = '/') {
  const router = useRouter()
  const route = useRoute()

  // 如果你是使用 router.getRoutes()（运行时路由），请用它；
  // 也可以改成读取一个传入的 constantRoutes（RouteRecordRaw[]）
  const allRoutes = computed<AnyRoute[]>(() => {
    // router.getRoutes() 返回 RouteRecordNormalized[]
    return router.getRoutes() as any as AnyRoute[]
  })

  // 找到 root 节点（在 router.getRoutes() 里通常存在）
  const root = computed<AnyRoute | undefined>(() =>
    allRoutes.value.find(r => r.path === rootPath)
  )

  const buildMenusFrom = (parent: AnyRoute): MenuItem[] => {
    const rawChildren = getChildren(parent).filter(r => !getMeta(r)?.hidden)
    // 注意：直接依赖 children 数组层级构建（避免 startsWith 导致的跨层级混淆）
    return rawChildren.map(r => {
      const itemPath = r.path?.startsWith('/') ? r.path : `${(parent.path === '/' ? '' : parent.path)}/${r.path}`.replace(/\/+/g, '/')
      const item: MenuItem = {
        key: String(getName(r) ?? itemPath),
        title: String(getMeta(r)?.title ?? getName(r) ?? itemPath),
        path: itemPath,
        icon: getMeta(r)?.icon
      }

      const visibleChildren = getChildren(r).filter(c => !getMeta(c)?.hidden)
      if (visibleChildren.length) {
        item.children = buildMenusFrom(r) // 递归，传子节点（AnyRoute）
      }
      return item
    })
  }

  const menus = computed<MenuItem[]>(() => {
    if (!root.value) return []
    return buildMenusFrom(root.value)
  })

  // activeKey 递归查找，支持多层嵌套
  const activeKey = computed(() => {
    const p = route.path
    const findKey = (list: MenuItem[]): string | undefined => {
      for (const it of list) {
        if (p === it.path || p.startsWith(it.path + '/')) return it.key
        if (it.children) {
          const k = findKey(it.children)
          if (k) return k
        }
      }
      return undefined
    }
    return findKey(menus.value) ?? ''
  })

  return { menus, activeKey }
}
