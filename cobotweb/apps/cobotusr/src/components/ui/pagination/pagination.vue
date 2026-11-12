<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import { ChevronLeft, ChevronRight } from '@lucide/vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import type { PaginationProps, PaginationEmits } from './pagination.types'
const props = defineProps<PaginationProps>(),
    emit = defineEmits<PaginationEmits>()
const current = computed(() => props.page ?? props.pageNumber ?? 1)
const total = computed(() => Math.max(1, props.pageCount ?? props.totalPages ?? 1))
const size = computed(() => props.size ?? props.pageSize ?? 10)
const options = computed(() => props.sizeOptions ?? props.pageSizeOptions ?? [10, 20, 50])
const pages = computed(() =>
    Array.from(
        { length: Math.min(5, total.value) },
        (_, i) => Math.max(1, Math.min(current.value - 2, total.value - 4)) + i,
    ),
)
const sizeId = useId()
const jumpInput = ref('')
function setPage(value: number) {
    if (props.disabled || !Number.isSafeInteger(value)) return
    const next = Math.max(1, Math.min(total.value, value))
    if (next === current.value) return
    emit('page-change', next)
}
function setSize(value: unknown) {
    const next = Number(value)
    if (!props.disabled && options.value.includes(next) && next !== size.value)
        emit('update:size', next)
}
function jump() {
    if (/^\d+$/.test(jumpInput.value)) {
        setPage(Number(jumpInput.value))
        jumpInput.value = ''
    }
}
</script>
<template>
    <nav
        aria-label="列表分页"
        class="flex flex-wrap items-center justify-between gap-3 border-t border-border px-4 py-3"
    >
        <div class="flex items-center gap-2 text-[11px] text-muted-foreground">
            <label :for="sizeId">每页</label>
            <UiSelect
                :id="sizeId"
                :model-value="size"
                :disabled="disabled"
                class="!min-h-8 !w-[72px] !px-2 !text-xs"
                @update:model-value="setSize"
                ><option v-for="option in options" :key="option" :value="option">
                    {{ option }}
                </option></UiSelect
            >
            <span aria-live="polite" class="tabular-nums">第 {{ current }} / {{ total }} 页</span>
        </div>
        <div class="flex items-center gap-1">
            <UiButton
                variant="outline"
                size="icon"
                aria-label="上一页"
                :disabled="disabled || current <= 1"
                @click="setPage(current - 1)"
                ><ChevronLeft :size="16" aria-hidden="true" /></UiButton
            ><UiButton
                v-for="value in pages"
                :key="value"
                :variant="value === current ? 'secondary' : 'ghost'"
                size="sm"
                class="hidden min-w-8 sm:inline-flex"
                :aria-current="value === current ? 'page' : undefined"
                :aria-label="'第 ' + value + ' 页'"
                :disabled="disabled"
                @click="setPage(value)"
                >{{ value }}</UiButton
            ><UiButton
                variant="outline"
                size="icon"
                aria-label="下一页"
                :disabled="disabled || current >= total"
                @click="setPage(current + 1)"
                ><ChevronRight :size="16" aria-hidden="true"
            /></UiButton>
            <form class="ml-2 hidden items-center gap-1 lg:flex" @submit.prevent="jump">
                <UiInput
                    v-model="jumpInput"
                    inputmode="numeric"
                    aria-label="跳转页码"
                    class="!min-h-8 !w-14 !px-2 !text-xs"
                    :disabled="disabled"
                /><UiButton type="submit" variant="outline" size="sm" :disabled="disabled"
                    >跳转</UiButton
                >
            </form>
        </div>
    </nav>
</template>
