<script setup lang="ts">
import { computed } from 'vue'
import { useUserStore } from '@/pinia/stores/user'
import { useRoute, useRouter } from 'vue-router'
import { ArrowUpRight, Command } from '@lucide/vue'
import Logo from '@/layouts/components/Logo/index.vue'
import Tooltip from '@/components/ui/tooltip/tooltip.vue'
import { activeNavigation, navigationFor } from '@/router/navigation'
import { navigateWorkspace } from '@/workspace/coordinator'
import { useCommandStore } from '@/pinia/stores/command'
import { useShortcutLabel } from '@/composables/use-platform'

defineProps<{ collapsed?: boolean; mobile?: boolean }>()
const emit = defineEmits<{ navigate: [] }>()
const route = useRoute(),
    router = useRouter(),
    user = useUserStore(),
    command = useCommandStore(),
    { commandKey } = useShortcutLabel()
const navigationGroups = computed(() => navigationFor(user.centerSession))
const isActive = (name: string) =>
    route.name === name || activeNavigation(route.name)?.name === name
const roleLabel = computed(() =>
    user.centerSession?.administrator
        ? '企业管理员'
        : user.centerSession?.developer_enabled
            ? '开发者'
            : '企业成员',
)
const displayName = computed(
    () => user.userInfo?.nickname || user.userInfo?.username || '当前用户',
)
const avatarText = computed(() => displayName.value.slice(0, 1).toUpperCase())
async function navigate(event: MouseEvent, name: string) {
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return
    event.preventDefault()
    const outcome = await navigateWorkspace(router, { name })
    if (outcome === 'success' || outcome === 'unchanged') emit('navigate')
}
function openSearch() {
    command.show()
    emit('navigate')
}
</script>
<template>
    <div class="flex h-full min-h-0 flex-col">
        <Logo :collapsed="collapsed" />
        <nav class="sidebar-nav" aria-label="主导航">
            <div
                v-for="group in navigationGroups"
                :key="group.title"
                class="nav-group"
            >
                <p v-if="!collapsed" class="nav-group-title">{{ group.title }}</p>
                <Tooltip v-for="item in group.items" :key="item.name" :text="collapsed ? item.title : ''">
                    <router-link
                        :to="{ name: item.name }"
                        class="nav-link"
                        :class="{
                            'is-active': isActive(item.name),
                            'justify-center': collapsed,
                        }"
                        :aria-current="route.name === item.name ? 'page' : undefined"
                        :aria-label="collapsed ? item.title : undefined"
                        @click="navigate($event, item.name)"
                    >
                        <component :is="item.icon" :size="17" aria-hidden="true" />
                        <span v-if="!collapsed" class="truncate">{{ item.title }}</span>
                        <ArrowUpRight
                            v-if="!collapsed && isActive(item.name)"
                            :size="12"
                            class="ml-auto shrink-0 opacity-50"
                            aria-hidden="true"
                        />
                    </router-link>
                </Tooltip>
            </div>

            <div v-if="!collapsed" class="nav-group">
                <p class="nav-group-title">快捷工作流</p>
                <button type="button" class="nav-link w-full text-left" @click="openSearch">
                    <Command :size="17" aria-hidden="true" />
                    <span>搜索页面与操作</span>
                    <kbd class="kbd ml-auto">{{ commandKey }}</kbd>
                </button>
            </div>
        </nav>

        <div class="nav-footer">
            <div v-if="!collapsed" class="nav-user">
                <span class="nav-user-avatar">{{ avatarText }}</span>
                <span class="min-w-0 flex-1">
                    <span class="nav-user-name block">{{ displayName }}</span>
                    <span class="nav-user-meta block">{{ roleLabel }}</span>
                </span>
            </div>
            <div v-else class="flex justify-center">
                <span class="nav-user-avatar" :title="displayName">{{ avatarText }}</span>
            </div>
            <div v-if="!collapsed" class="sidebar-status">
                <span class="status-dot" aria-hidden="true" />
                身份服务已连接
            </div>
        </div>
    </div>
</template>
