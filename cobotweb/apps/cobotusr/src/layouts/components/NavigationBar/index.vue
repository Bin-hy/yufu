<script setup lang="ts">
import { ChevronDown, LogOut, Menu, PanelLeft, Search, UserRound } from '@lucide/vue'
import { DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui'
import { useRouter } from 'vue-router'
import ThemeSwitcher from '@/components/ui/theme-switcher/theme-switcher.vue'
import DropdownMenu from '@/components/ui/dropdown-menu/dropdown-menu.vue'
import Breadcrumb from '@/layouts/components/Breadcrumb/index.vue'
import { useLayoutStore } from '@/pinia/stores/layout'
import { useUserStore } from '@/pinia/stores/user'
import { useCommandStore } from '@/pinia/stores/command'
import { logoutAndRedirect } from '@/common/utils/auth'
import { useTheme } from '@/composables/use-theme'
import { useShortcutLabel } from '@/composables/use-platform'
const { setTheme } = useTheme(),
    { commandKey } = useShortcutLabel()
const layout = useLayoutStore(),
    user = useUserStore(),
    command = useCommandStore(),
    router = useRouter()
const displayName = () => user.userInfo?.nickname || user.userInfo?.username || '当前用户'
const avatar = () => displayName().slice(0, 1).toUpperCase()
</script>
<template>
    <header class="workspace-topbar">
        <div class="flex min-w-0 items-center gap-2">
            <button
                type="button"
                class="icon-button lg:hidden"
                aria-label="打开导航"
                @click="layout.mobileOpen = true"
            >
                <Menu :size="18" aria-hidden="true" />
            </button>
            <button
                type="button"
                class="icon-button hidden lg:inline-flex"
                aria-label="切换侧栏"
                :aria-pressed="layout.sidebarCollapsed"
                @click="layout.toggleSidebar"
            >
                <PanelLeft :size="17" aria-hidden="true" />
            </button>
            <Breadcrumb />
        </div>
        <div class="flex shrink-0 items-center gap-2 sm:gap-3">
            <button type="button" class="topbar-search" aria-label="搜索页面与操作" @click="command.show()">
                <Search :size="14" aria-hidden="true" />
                <span>搜索</span>
                <kbd>{{ commandKey }}</kbd>
            </button>
            <ThemeSwitcher />
            <DropdownMenu>
                <template #trigger>
                    <button class="user-trigger" type="button" aria-label="用户菜单">
                        <span class="user-avatar">{{ avatar() }}</span>
                        <span class="user-name hidden sm:block">{{ displayName() }}</span>
                        <ChevronDown :size="13" class="hidden text-muted-foreground sm:block" aria-hidden="true" />
                    </button>
                </template>
                <div class="px-2.5 py-2">
                    <p class="text-xs font-semibold">{{ displayName() }}</p>
                    <p class="mt-0.5 text-[10.5px] text-muted-foreground">
                        {{ user.centerSession?.username }} · {{ user.centerSession?.administrator ? '企业管理员' : user.centerSession?.developer_enabled ? '开发者' : '企业成员' }}
                    </p>
                </div>
                <DropdownMenuSeparator class="dropdown-separator" />
                <DropdownMenuItem as-child>
                    <router-link to="/account" class="dropdown-item">
                        <UserRound :size="15" aria-hidden="true" />账户资料
                    </router-link>
                </DropdownMenuItem>
                <DropdownMenuItem as-child>
                    <router-link to="/account/sessions" class="dropdown-item">
                        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect width="18" height="11" x="3" y="11" rx="2" ry="2"></rect><path d="M7 11V7a5 5 0 0 1 10 0v4"></path></svg>登录会话
                    </router-link>
                </DropdownMenuItem>
                <DropdownMenuSeparator class="dropdown-separator sm:hidden" />
                <DropdownMenuItem class="dropdown-item sm:hidden" @select="setTheme('light')"
                    >主题：浅色</DropdownMenuItem
                >
                <DropdownMenuItem class="dropdown-item sm:hidden" @select="setTheme('dark')"
                    >主题：暗色</DropdownMenuItem
                >
                <DropdownMenuItem class="dropdown-item sm:hidden" @select="setTheme('system')"
                    >主题：跟随系统</DropdownMenuItem
                >
                <DropdownMenuSeparator class="dropdown-separator" />
                <DropdownMenuItem
                    class="dropdown-item text-destructive"
                    @select="logoutAndRedirect(router)"
                    ><LogOut :size="15" aria-hidden="true" />退出登录</DropdownMenuItem
                >
            </DropdownMenu>
        </div>
    </header>
</template>
