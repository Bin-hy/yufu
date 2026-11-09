<script setup lang="ts">
import { computed } from 'vue'
import { useUserStore } from '@/pinia/stores/user'
import { useRoute, useRouter } from 'vue-router'
import { DropdownMenuItem } from 'reka-ui'
import { ChevronsLeft, ChevronsRight, ArrowUpRight } from '@lucide/vue'
import Logo from '@/layouts/components/Logo/index.vue'
import Tooltip from '@/components/ui/tooltip/tooltip.vue'
import DropdownMenu from '@/components/ui/dropdown-menu/dropdown-menu.vue'
import { navigationFor, assignmentLinks, activeNavigation } from '@/router/navigation'
import { navigateWorkspace } from '@/workspace/coordinator'
import { useLayoutStore } from '@/pinia/stores/layout'
defineProps<{ collapsed?: boolean; mobile?: boolean }>()
const emit = defineEmits<{ navigate: [] }>()
const route = useRoute(),
    router = useRouter(),
    layout = useLayoutStore()
const user = useUserStore()
const navigationGroups = computed(() => navigationFor(user.centerSession))
async function navigate(event: MouseEvent, name: string) {
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return
    event.preventDefault()
    const outcome = await navigateWorkspace(router, { name })
    if (outcome === 'success' || outcome === 'unchanged') emit('navigate')
}
</script>
<template>
    <div class="flex h-full min-h-0 flex-col">
        <Logo :collapsed="collapsed" />
        <nav class="flex-1 overflow-y-auto pb-4">
            <div
                v-for="group in navigationGroups"
                :key="group.title"
                class="nav-group"
                :class="{ '!px-2': collapsed }"
            >
                <p v-if="!collapsed" class="nav-group-title">{{ group.title }}</p>
                <template v-for="item in group.items" :key="item.name"
                    ><template v-if="collapsed && 'children' in item"
                        ><DropdownMenu
                            ><template #trigger
                                ><button
                                    class="nav-link w-full justify-center"
                                    :class="{
                                        'is-active':
                                            activeNavigation(route.name)?.name === item.name,
                                    }"
                                    :aria-label="item.title"
                                    type="button"
                                >
                                    <component
                                        :is="item.icon"
                                        :size="18"
                                        aria-hidden="true"
                                    /></button></template
                            ><DropdownMenuItem as-child
                                ><router-link
                                    class="dropdown-item"
                                    :to="{ name: item.name }"
                                    @click="navigate($event, item.name)"
                                    >{{ item.title }}</router-link
                                ></DropdownMenuItem
                            ><DropdownMenuItem v-for="child in item.children" :key="child" as-child
                                ><router-link
                                    class="dropdown-item"
                                    :to="{ name: child }"
                                    @click="navigate($event, child)"
                                    >{{ router.resolve({ name: child }).meta.title }}</router-link
                                ></DropdownMenuItem
                            ></DropdownMenu
                        ></template
                    ><Tooltip v-else-if="collapsed" :text="item.title"
                        ><router-link
                            :to="{ name: item.name }"
                            class="nav-link justify-center"
                            :class="{
                                'is-active': activeNavigation(route.name)?.name === item.name,
                            }"
                            :aria-label="item.title"
                            @click="navigate($event, item.name)"
                            ><component
                                :is="item.icon"
                                :size="18"
                                aria-hidden="true" /></router-link></Tooltip
                    ><template v-else
                        ><router-link
                            :to="{ name: item.name }"
                            class="nav-link"
                            :class="{
                                'is-active': activeNavigation(route.name)?.name === item.name,
                            }"
                            :aria-current="route.name === item.name ? 'page' : undefined"
                            @click="navigate($event, item.name)"
                            ><component :is="item.icon" :size="18" aria-hidden="true" /><span>{{
                                item.title
                            }}</span></router-link
                        ><router-link
                            v-for="child in 'children' in item ? item.children : []"
                            :key="child"
                            :to="{ name: child }"
                            class="nav-sublink"
                            :class="{ 'is-active': route.name === child }"
                            @click="navigate($event, child)"
                            >{{ router.resolve({ name: child }).meta.title }}</router-link
                        ></template
                    ></template
                >
            </div>
            <div v-if="!collapsed" class="nav-group">
                <p class="nav-group-title">快捷工作流</p>
                <router-link
                    v-for="link in assignmentLinks"
                    :key="link.name"
                    :to="{ name: link.name }"
                    class="nav-link"
                    @click="navigate($event, link.name)"
                    ><component :is="link.icon" :size="17" aria-hidden="true" />{{ link.title
                    }}<ArrowUpRight :size="13" class="ml-auto" aria-hidden="true"
                /></router-link>
            </div>
        </nav>
        <div class="nav-footer">
            <button
                v-if="!mobile"
                type="button"
                class="flex min-h-9 w-full items-center gap-2 rounded px-1"
                aria-label="切换侧栏"
                @click="layout.toggleSidebar"
            >
                <component
                    :is="collapsed ? ChevronsRight : ChevronsLeft"
                    :size="16"
                    aria-hidden="true"
                /><span v-if="!collapsed">收起导航</span>
            </button>
            <p v-if="!collapsed" class="mt-3">身份 · 权限 · 应用</p>
        </div>
    </div>
</template>
