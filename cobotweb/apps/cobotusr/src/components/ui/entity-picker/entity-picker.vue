<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'
import { Check, ChevronsUpDown, LoaderCircle, Search, X } from '@lucide/vue'
import type { PickerLoader, PickerOption } from './types'

const props = withDefaults(
    defineProps<{
        modelValue?: string | number | null
        selectedLabel?: string
        selectedSecondary?: string
        load: PickerLoader
        placeholder?: string
        emptyText?: string
        disabled?: boolean
        invalid?: boolean
        minChars?: number
        id?: string
    }>(),
    {
        modelValue: null,
        selectedLabel: '',
        selectedSecondary: '',
        placeholder: '输入名称、用户名或 ID 搜索…',
        emptyText: '没有匹配的对象',
        disabled: false,
        invalid: false,
        minChars: 0,
    },
)
const emit = defineEmits<{
    'update:modelValue': [value: string | number | null]
    select: [option: PickerOption]
}>()

const root = ref<HTMLElement | null>(null)
const input = ref<HTMLInputElement | null>(null)
const generatedId = useId()
const menuStyle = ref<Record<string, string>>({})
const open = ref(false)
const query = ref('')
const options = ref<PickerOption[]>([])
const loading = ref(false)
const error = ref('')
const active = ref(-1)
let controller: AbortController | null = null
let sequence = 0
let debounceTimer: number | undefined

const hasSelection = computed(
    () => props.modelValue !== null && props.modelValue !== undefined && props.modelValue !== '',
)
const selectedText = computed(
    () => props.selectedLabel || (hasSelection.value ? '#' + String(props.modelValue) : ''),
)
const selectedInitial = computed(() => selectedText.value.slice(0, 1).toUpperCase())
const listId = computed(() => (props.id ? props.id + '-listbox' : generatedId + '-listbox'))

function close() {
    window.clearTimeout(debounceTimer)
    open.value = false
    query.value = ''
    active.value = -1
}

async function search() {
    if (props.disabled || !open.value) return
    const keyword = query.value.trim()
    if (keyword.length < props.minChars) {
        options.value = []
        return
    }
    sequence++
    const id = sequence
    controller?.abort()
    controller = new AbortController()
    loading.value = true
    error.value = ''
    try {
        const result = await props.load(keyword, controller.signal)
        if (id !== sequence || controller.signal.aborted) return
        options.value = result
        active.value = result.findIndex((option) => !option.disabled)
    } catch (reason) {
        if (id !== sequence || controller.signal.aborted) return
        options.value = []
        error.value = reason instanceof Error ? reason.message : '搜索失败，请重试'
    } finally {
        if (id === sequence) loading.value = false
    }
}

watch(query, () => {
    window.clearTimeout(debounceTimer)
    // A new query invalidates the visible result set immediately. Keeping the
    // previous options clickable during the debounce window lets a stale row
    // win the race against the request the user just typed.
    controller?.abort()
    controller = null
    sequence++
    options.value = []
    active.value = -1
    debounceTimer = window.setTimeout(() => void search(), 220)
})

function updatePosition() {
    const rect = root.value?.getBoundingClientRect()
    if (!rect) return
    const below = window.innerHeight - rect.bottom
    const openAbove = below < 260 && rect.top > below
    menuStyle.value = {
        position: 'fixed',
        top: Math.round(openAbove ? Math.max(8, rect.top - 306) : rect.bottom + 6) + 'px',
        left: Math.round(rect.left) + 'px',
        width: Math.round(rect.width) + 'px',
        maxHeight: Math.min(300, window.innerHeight - 16) + 'px',
    }
}

async function openPicker() {
    if (props.disabled) return
    open.value = true
    await nextTick()
    updatePosition()
    input.value?.focus()
    if (!options.value.length && !loading.value) void search()
}

function choose(option: PickerOption) {
    if (option.disabled) return
    emit('update:modelValue', option.value)
    emit('select', option)
    close()
}

function move(delta: number) {
    if (!options.value.length) return
    let next = active.value
    for (let step = 0; step < options.value.length; step++) {
        next = (next + delta + options.value.length) % options.value.length
        if (!options.value[next]?.disabled) {
            active.value = next
            return
        }
    }
}

function onKeydown(event: KeyboardEvent) {
    if (!open.value && ['ArrowDown', 'Enter', ' '].includes(event.key)) {
        event.preventDefault()
        void openPicker()
        return
    }
    if (event.key === 'ArrowDown') {
        event.preventDefault()
        move(1)
    } else if (event.key === 'ArrowUp') {
        event.preventDefault()
        move(-1)
    } else if (event.key === 'Enter') {
        event.preventDefault()
        choose(options.value[active.value])
    } else if (event.key === 'Escape') {
        event.preventDefault()
        close()
    }
}

function clear(event: MouseEvent) {
    event.stopPropagation()
    emit('update:modelValue', null)
    query.value = ''
    options.value = []
    close()
}

function replace(event: Event) {
    event.stopPropagation()
    query.value = ''
    options.value = []
    void openPicker()
}

function onPointerDown(event: PointerEvent) {
    const target = event.target as Element | null
    if (!target) return
    if (root.value?.contains(target) || target.closest('.entity-picker-menu')) return
    close()
}
function onViewportChange() {
    if (open.value) updatePosition()
}

onMounted(() => {
    document.addEventListener('pointerdown', onPointerDown)
    window.addEventListener('resize', onViewportChange)
    window.addEventListener('scroll', onViewportChange, true)
})
onBeforeUnmount(() => {
    controller?.abort()
    window.clearTimeout(debounceTimer)
    document.removeEventListener('pointerdown', onPointerDown)
    window.removeEventListener('resize', onViewportChange)
    window.removeEventListener('scroll', onViewportChange, true)
})
</script>

<template>
    <div ref="root" class="entity-picker">
        <div
            v-if="hasSelection && !open"
            class="entity-picker-selected"
            :class="{ 'is-invalid': invalid, 'is-disabled': disabled }"
            tabindex="0"
            role="button"
            :aria-label="'已选择 ' + selectedText + '，点击更换'"
            @click="replace"
            @keydown.enter.prevent="replace"
            @keydown.space.prevent="replace"
        >
            <span class="entity-picker-avatar">{{ selectedInitial }}</span>
            <span class="min-w-0 flex-1">
                <span class="entity-picker-selected-label">{{ selectedText }}</span>
                <span v-if="selectedSecondary" class="entity-picker-selected-meta">
                    {{ selectedSecondary }}
                </span>
            </span>
            <span class="entity-picker-change">更换</span>
            <button
                v-if="!disabled"
                type="button"
                class="icon-button !h-7 !w-7"
                aria-label="清除选择"
                title="清除选择"
                @click="clear"
            >
                <X :size="13" aria-hidden="true" />
            </button>
        </div>

        <div
            v-else
            class="ui-input-shell entity-picker-input"
            :class="{ 'is-invalid': invalid, 'is-disabled': disabled }"
            @click="openPicker"
        >
            <Search :size="14" class="ml-2.5 shrink-0 text-muted-foreground" aria-hidden="true" />
            <input
                :id="id"
                ref="input"
                v-model="query"
                type="text"
                class="ui-input"
                role="combobox"
                :placeholder="placeholder"
                :disabled="disabled"
                :aria-expanded="open"
                :aria-controls="listId"
                :aria-activedescendant="
                    active >= 0 ? listId + '-' + active : undefined
                "
                aria-autocomplete="list"
                autocomplete="off"
                spellcheck="false"
                @focus="openPicker"
                @keydown="onKeydown"
            />
            <LoaderCircle
                v-if="loading"
                :size="14"
                class="mr-2 shrink-0 animate-spin text-muted-foreground"
                aria-hidden="true"
            />
            <ChevronsUpDown
                v-else
                :size="14"
                class="mr-2 shrink-0 text-muted-foreground"
                aria-hidden="true"
            />
        </div>

        <Teleport to="body">
        <div
            v-if="open"
            :id="listId"
            class="entity-picker-menu is-floating"
            :style="menuStyle"
            role="listbox"
        >
            <p v-if="loading && !options.length" class="entity-picker-state" role="status">
                正在搜索…
            </p>
            <p v-else-if="error" class="entity-picker-state text-destructive" role="alert">
                {{ error }}
            </p>
            <p v-else-if="!options.length" class="entity-picker-state">{{ emptyText }}</p>
            <template v-else>
                <button
                    v-for="(option, index) in options"
                    :id="listId + '-' + index"
                    :key="String(option.value)"
                    type="button"
                    class="entity-picker-option"
                    :class="{ 'is-active': index === active }"
                    :data-active="index === active"
                    :data-value="String(option.value)"
                    role="option"
                    :aria-selected="option.value === modelValue"
                    :disabled="option.disabled"
                    @mouseenter="active = index"
                    @click="choose(option)"
                >
                    <span class="entity-picker-option-avatar">
                        {{ option.label.slice(0, 1).toUpperCase() }}
                    </span>
                    <span class="min-w-0 flex-1 text-left">
                        <span class="entity-picker-option-label">{{ option.label }}</span>
                        <span v-if="option.secondary" class="entity-picker-option-meta">
                            {{ option.secondary }}
                        </span>
                    </span>
                    <Check
                        v-if="option.value === modelValue"
                        :size="15"
                        class="shrink-0 text-primary"
                        aria-hidden="true"
                    />
                </button>
            </template>
            <div class="entity-picker-hint">
                <span><kbd class="kbd">↑</kbd><kbd class="kbd">↓</kbd> 选择</span>
                <span><kbd class="kbd">↵</kbd> 确认</span>
                <span><kbd class="kbd">esc</kbd> 关闭</span>
            </div>
        </div>
        </Teleport>
    </div>
</template>
