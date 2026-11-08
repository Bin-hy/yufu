<script setup lang="ts">
import { Menu, ChevronDown, LogOut, UserRound, PanelLeft } from '@lucide/vue'
import { DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui'
import { useRouter } from 'vue-router'
import ThemeSwitcher from '@/components/ui/theme-switcher/theme-switcher.vue'
import DropdownMenu from '@/components/ui/dropdown-menu/dropdown-menu.vue'
import Breadcrumb from '@/layouts/components/Breadcrumb/index.vue'
import { useLayoutStore } from '@/pinia/stores/layout'
import { useUserStore } from '@/pinia/stores/user'
import { logoutAndRedirect } from '@/common/utils/auth'
import { useTheme } from '@/composables/use-theme'
const { setTheme } = useTheme()
const layout = useLayoutStore(),
    user = useUserStore(),
    router = useRouter()
</script>
<template>
    <header class="workspace-topbar">
        <div class="flex min-w-0 items-center gap-3">
            <button
                type="button"
                class="icon-button lg:hidden"
                aria-label="打开导航"
                @click="layout.mobileOpen = true"
            >
                <Menu :size="19" aria-hidden="true" /></button
            ><button
                type="button"
                class="icon-button hidden lg:inline-flex"
                aria-label="切换侧栏"
                @click="layout.toggleSidebar"
            >
                <PanelLeft :size="17" aria-hidden="true" /></button
            ><Breadcrumb />
        </div>
        <div class="flex shrink-0 items-center gap-4">
            <ThemeSwitcher /><DropdownMenu
                ><template #trigger
                    ><button
                        class="flex min-h-11 min-w-11 sm:min-h-9 items-center justify-center gap-2 text-xs"
                        type="button"
                        aria-label="用户菜单"
                    >
                        <span
                            class="flex h-7 w-7 items-center justify-center rounded-full bg-secondary text-xs font-semibold text-secondary-foreground"
                            >{{
                                (user.userInfo?.nickname || user.userInfo?.username || 'U').slice(
                                    0,
                                    1,
                                )
                            }}</span
                        ><span class="hidden max-w-32 truncate sm:block">{{
                            user.userInfo?.nickname || user.userInfo?.username || '当前用户'
                        }}</span
                        ><ChevronDown :size="14" aria-hidden="true" /></button></template
                ><DropdownMenuItem as-child
                    ><router-link to="/profile" class="dropdown-item"
                        ><UserRound :size="15" aria-hidden="true" />个人中心</router-link
                    ></DropdownMenuItem
                ><DropdownMenuItem class="dropdown-item sm:hidden" @select="setTheme('light')"
                    >主题：浅色</DropdownMenuItem
                >
                <DropdownMenuItem class="dropdown-item sm:hidden" @select="setTheme('dark')"
                    >主题：暗色</DropdownMenuItem
                >
                <DropdownMenuItem class="dropdown-item sm:hidden" @select="setTheme('system')"
                    >主题：跟随系统</DropdownMenuItem
                >
                <DropdownMenuSeparator class="my-1 h-px bg-border" /><DropdownMenuItem
                    class="dropdown-item text-destructive"
                    @select="logoutAndRedirect(router)"
                    ><LogOut :size="15" aria-hidden="true" />退出登录</DropdownMenuItem
                ></DropdownMenu
            >
        </div>
    </header>
</template>
