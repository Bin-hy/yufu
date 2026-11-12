<script setup lang="ts">
import { computed, isVNode, nextTick, onBeforeUnmount, ref, watch, type Component } from 'vue'
import { useRouter } from 'vue-router'
import { useTabsStore } from '@/pinia/stores/tabs'
import { getPageComponent } from '@/workspace/page-registry'
import { sessionEpoch, sessionOwner } from '@/workspace/session'
import { saveWorkspace } from '@/storage/workspace'
import type { RouteLocationNormalizedLoaded } from 'vue-router'
const tabs = useTabsStore()
const router = useRouter()
const scroll = ref<HTMLElement | null>(null)
let pendingScroll: number | null = null
let observer: ResizeObserver | null = null
let saveTimer: ReturnType<typeof setTimeout> | null = null
function persistScroll() {
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = null
    saveWorkspace(router, sessionOwner.value, tabs.visited, tabs.activeKey)
}
function restoreScroll() {
    const main = scroll.value
    if (!main || pendingScroll === null) return
    const maximum = Math.max(0, main.scrollHeight - main.clientHeight)
    main.scrollTop = Math.min(pendingScroll, maximum)
    if (maximum >= pendingScroll) pendingScroll = null
}
function stopRestoring() {
    pendingScroll = null
}
const include = computed(() => tabs.visited.map((tab) => tab.cacheName))
function page(component: Component, route: RouteLocationNormalizedLoaded) {
    const tab = tabs.visited.find((tab) => tab.key === tabs.activeKey)
    if (!component || !tab) return null
    const leaf = isVNode(component) ? (component.type as Component) : component
    return getPageComponent(tab, leaf, route, () => tabs.activeKey)
}
watch(
    () => [tabs.activeKey, sessionEpoch.value] as const,
    async ([key], previous) => {
        const old = tabs.visited.find((tab) => tab.key === previous?.[0])
        if (old && scroll.value && pendingScroll === null) old.scrollTop = scroll.value.scrollTop
        observer?.disconnect()
        pendingScroll = tabs.visited.find((tab) => tab.key === key)?.scrollTop ?? 0
        await nextTick()
        if (key !== tabs.activeKey) return
        restoreScroll()
        const content = scroll.value?.firstElementChild
        if (content && typeof ResizeObserver !== 'undefined') {
            observer = new ResizeObserver(restoreScroll)
            observer.observe(content)
        }
    },
    { immediate: true },
)
function saveScroll() {
    const tab = tabs.visited.find((tab) => tab.key === tabs.activeKey)
    if (tab && scroll.value && pendingScroll === null) {
        tab.scrollTop = scroll.value.scrollTop
        if (saveTimer) clearTimeout(saveTimer)
        saveTimer = setTimeout(persistScroll, 150)
    }
}
window.addEventListener('pagehide', persistScroll)
onBeforeUnmount(() => {
    observer?.disconnect()
    if (saveTimer) clearTimeout(saveTimer)
    window.removeEventListener('pagehide', persistScroll)
})
</script>
<template>
    <main
        id="workspace-content"
        ref="scroll"
        class="workspace-content"
        tabindex="-1"
        @scroll.passive="saveScroll"
        @wheel.passive="stopRestoring"
        @touchmove.passive="stopRestoring"
        @keydown="stopRestoring"
    >
        <router-view v-slot="{ Component, route }"
            ><KeepAlive :key="sessionEpoch" :include="include"
                ><component
                    :is="page(Component, route)"
                    v-if="Component"
                    :key="tabs.activeKey" /></KeepAlive
        ></router-view>
    </main>
</template>
