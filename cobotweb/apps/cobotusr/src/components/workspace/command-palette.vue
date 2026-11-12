<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle } from 'reka-ui'
import { CornerDownLeft, LogOut, Monitor, Moon, Search, Sun } from '@lucide/vue'
import { navigationFor } from '@/router/navigation'
import { useCommandStore } from '@/pinia/stores/command'
import { useUserStore } from '@/pinia/stores/user'
import { useTheme } from '@/composables/use-theme'
import { logoutAndRedirect } from '@/common/utils/auth'
import { navigateWorkspace } from '@/workspace/coordinator'

type CommandItem = {
    id: string
    label: string
    group: string
    icon: unknown
    run: () => void | Promise<void>
    meta?: string
}

const command = useCommandStore(),
    user = useUserStore(),
    router = useRouter(),
    { setTheme } = useTheme()
const query = ref(''),
    active = ref(0),
    input = ref<HTMLInputElement | null>(null)

const navigationCommands = computed<CommandItem[]>(() =>
    navigationFor(user.centerSession).flatMap((group) =>
        group.items.map((item) => ({
            id: 'nav:' + item.name,
            label: item.title,
            group: group.title,
            icon: item.icon,
            meta: item.name === router.currentRoute.value.name ? '当前页面' : undefined,
            run: async () => {
                command.hide()
                if (router.currentRoute.value.name === item.name) return
                await navigateWorkspace(router, { name: item.name })
            },
        })),
    ),
)

const actionCommands: CommandItem[] = [
    {
        id: 'theme:light',
        label: '切换到浅色主题',
        group: '外观',
        icon: Sun,
        run: () => {
            setTheme('light')
            command.hide()
        },
    },
    {
        id: 'theme:dark',
        label: '切换到暗色主题',
        group: '外观',
        icon: Moon,
        run: () => {
            setTheme('dark')
            command.hide()
        },
    },
    {
        id: 'theme:system',
        label: '跟随系统主题',
        group: '外观',
        icon: Monitor,
        run: () => {
            setTheme('system')
            command.hide()
        },
    },
    {
        id: 'session:logout',
        label: '退出登录',
        group: '账户',
        icon: LogOut,
        run: async () => {
            command.hide()
            await logoutAndRedirect(router)
        },
    },
]

const items = computed<CommandItem[]>(() => {
    const keyword = query.value.trim().toLowerCase()
    const all = [...navigationCommands.value, ...actionCommands]
    if (!keyword) return all
    return all.filter(
        (item) =>
            item.label.toLowerCase().includes(keyword) ||
            item.group.toLowerCase().includes(keyword),
    )
})

const grouped = computed(() => {
    const groups: { title: string; items: CommandItem[] }[] = []
    for (const item of items.value) {
        const group = groups.find((entry) => entry.title === item.group)
        if (group) group.items.push(item)
        else groups.push({ title: item.group, items: [item] })
    }
    return groups
})

function indexOf(item: CommandItem) {
    return items.value.findIndex((entry) => entry.id === item.id)
}

watch(query, () => {
    active.value = 0
})

watch(
    () => command.open,
    async (open) => {
        if (open) {
            query.value = ''
            active.value = 0
            await nextTick()
            input.value?.focus()
        }
    },
)

function move(delta: number) {
    if (!items.value.length) return
    active.value = (active.value + delta + items.value.length) % items.value.length
}

function choose(item: CommandItem | undefined) {
    if (item) void item.run()
}

function onKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        command.toggle()
    }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
    <DialogRoot :open="command.open" @update:open="(value) => (value ? command.show() : command.hide())">
        <DialogPortal>
            <DialogOverlay class="command-overlay" />
            <DialogContent class="command-panel" @open-auto-focus.prevent>
                <DialogTitle class="sr-only">搜索页面与操作</DialogTitle>
                <DialogDescription class="sr-only">
                    输入关键词筛选页面或操作，使用上下方向键选择，回车打开。
                </DialogDescription>
                <div class="command-input-row">
                    <Search :size="18" aria-hidden="true" />
                    <input
                        ref="input"
                        v-model="query"
                        class="command-input"
                        type="text"
                        placeholder="搜索页面、功能或操作…"
                        aria-label="搜索页面与操作"
                        @keydown.down.prevent="move(1)"
                        @keydown.up.prevent="move(-1)"
                        @keydown.enter.prevent="choose(items[active])"
                    />
                    <kbd class="kbd">ESC</kbd>
                </div>
                <div class="command-list" role="listbox" aria-label="可用页面与操作">
                    <template v-if="items.length">
                        <section v-for="group in grouped" :key="group.title">
                            <p class="command-group-label">{{ group.title }}</p>
                            <button
                                v-for="item in group.items"
                                :key="item.id"
                                type="button"
                                class="command-item"
                                :data-active="indexOf(item) === active"
                                role="option"
                                :aria-selected="indexOf(item) === active"
                                @mouseenter="active = indexOf(item)"
                                @click="choose(item)"
                            >
                                <span class="command-item-icon">
                                    <component :is="item.icon" :size="14" aria-hidden="true" />
                                </span>
                                <span class="command-item-label">{{ item.label }}</span>
                                <span v-if="item.meta" class="command-item-meta">{{ item.meta }}</span>
                                <CornerDownLeft
                                    v-if="indexOf(item) === active"
                                    :size="13"
                                    class="text-muted-foreground"
                                    aria-hidden="true"
                                />
                            </button>
                        </section>
                    </template>
                    <p v-else class="command-empty">没有找到匹配的页面或操作</p>
                </div>
                <footer class="command-footer">
                    <span class="flex items-center gap-3">
                        <span><kbd class="kbd">↑</kbd> <kbd class="kbd">↓</kbd> 选择</span>
                        <span><kbd class="kbd">↵</kbd> 打开</span>
                    </span>
                    <span>输入关键词快速跳转</span>
                </footer>
            </DialogContent>
        </DialogPortal>
    </DialogRoot>
</template>
