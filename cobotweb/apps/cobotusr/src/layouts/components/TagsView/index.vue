<script setup lang="ts">
import { watch, nextTick, ref } from 'vue'
import { useRouter } from 'vue-router'
import { X, MoreHorizontal, House } from '@lucide/vue'
import { DropdownMenuItem } from 'reka-ui'
import DropdownMenu from '@/components/ui/dropdown-menu/dropdown-menu.vue'
import { useTabsStore } from '@/pinia/stores/tabs'
import { closeWorkspaceTab } from '@/workspace/coordinator'
const tabs = useTabsStore(),
    router = useRouter(),
    scroller = ref<HTMLElement | null>(null)
watch(
    () => tabs.activeKey,
    () => {
        void nextTick(() =>
            scroller.value
                ?.querySelector('[aria-current=page]')
                ?.scrollIntoView({ block: 'nearest', inline: 'nearest' }),
        )
    },
)
</script>
<template>
    <nav class="tags-bar" aria-label="已打开页面">
        <div ref="scroller" class="tags-scroll">
            <div
                v-for="tab in tabs.visited"
                :key="tab.key"
                class="workspace-tab"
                :class="{ 'is-active': tabs.activeKey === tab.key }"
            >
                <router-link
                    :to="tab.fullPath"
                    :aria-current="tabs.activeKey === tab.key ? 'page' : undefined"
                    ><House v-if="tab.affix" :size="12" class="mr-1 inline" aria-hidden="true" />{{
                        tab.title
                    }}</router-link
                ><button
                    v-if="!tab.affix"
                    type="button"
                    :aria-label="'关闭' + tab.title"
                    @click="closeWorkspaceTab(router, tab.key)"
                >
                    <X :size="12" aria-hidden="true" />
                </button>
            </div>
        </div>
        <DropdownMenu
            ><template #trigger
                ><button type="button" class="icon-button !h-7 !w-7" aria-label="页面标签操作">
                    <MoreHorizontal :size="16" aria-hidden="true" /></button></template
            ><DropdownMenuItem
                class="dropdown-item"
                :disabled="tabs.activeKey === 'dashboard-home'"
                @select="closeWorkspaceTab(router, tabs.activeKey)"
                >关闭当前</DropdownMenuItem
            ><DropdownMenuItem
                class="dropdown-item"
                @select="closeWorkspaceTab(router, tabs.activeKey, 'others')"
                >关闭其他</DropdownMenuItem
            ><DropdownMenuItem
                class="dropdown-item"
                @select="closeWorkspaceTab(router, tabs.activeKey, 'right')"
                >关闭右侧</DropdownMenuItem
            ></DropdownMenu
        >
    </nav>
</template>
