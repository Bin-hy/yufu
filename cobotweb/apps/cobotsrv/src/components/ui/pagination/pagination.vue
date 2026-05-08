<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import type { PaginationProps, PaginationEmits } from './pagination.types'

const props = defineProps<PaginationProps>()
const emits = defineEmits<PaginationEmits>()

// ========== 归一化（兼容旧版与新版命名） ==========
const currentPage = computed(() => {
  return props.page ?? props.pageNumber ?? 1
})
const pageCount = computed(() => {
  return props.pageCount ?? props.totalPages ?? 1
})
const pageSizeVal = computed(() => {
  return props.size ?? props.pageSize
})
const totalItemsVal = computed(() => {
  return props.total ?? props.totalItems
})

// ========== 计算属性 ==========
const total = computed(() => Math.max(1, pageCount.value))
const canPrev = computed(() => currentPage.value > 1)
const canNext = computed(() => currentPage.value < total.value)
const sizeOptions = computed(() => props.sizeOptions ?? props.pageSizeOptions ?? [10, 20, 50])

// ========== 分页跳转逻辑 ==========
function setPage(p: number) {
  if (props.disabled) return
  const next = Math.min(Math.max(1, p), total.value)
  if (next !== currentPage.value) {
    // 旧版事件
    emits('update:pageNumber', next)
    emits('change', next)
    // 新版事件
    emits('update:page', next)
    emits('page-change', next)
  }
}

const prev = () => canPrev.value && setPage(currentPage.value - 1)
const next = () => canNext.value && setPage(currentPage.value + 1)
const first = () => setPage(1)
const last = () => setPage(total.value)

// ========== 页码范围（半径可调） ==========
const pages = computed(() => {
  const radius = 2
  const start = Math.max(1, currentPage.value - radius)
  const end = Math.min(total.value, currentPage.value + radius)
  return Array.from({ length: end - start + 1 }, (_, i) => start + i)
})

// 省略号显示控制
const showFirst = computed(() => pages.value[0] > 1)
const showLast = computed(() => pages.value[pages.value.length - 1] < total.value)

// ========== 跳页 ==========
const jumpInput = ref('')
const jump = () => {
  const num = Number(jumpInput.value)
  if (!Number.isFinite(num)) return
  setPage(num)
  jumpInput.value = ''
}

// ========== 页大小 ==========
const setSize = (size: number) => {
  if (props.disabled) return
  // 旧版事件
  emits('update:pageSize', size)
  emits('size-change', size)
  // 新版事件
  emits('update:size', size)
}
</script>

<template>
  <div class="flex flex-wrap items-center justify-between gap-3 mt-3 select-none">
    <!-- 左侧：页容量与信息 -->
    <div class="flex items-center gap-3">
      <div v-if="pageSizeVal !== undefined" class="flex items-center gap-1.5">
        <span class="text-xs text-muted-foreground">每页</span>
        <UiSelect :model-value="pageSizeVal" class="h-8 w-20 px-2 text-xs" @update:modelValue="(v:any) => setSize(Number(v))">
          <option v-for="opt in sizeOptions" :key="opt" :value="opt">{{ opt }}</option>
        </UiSelect>
        <span class="text-xs text-muted-foreground">条</span>
      </div>
      <div class="hidden sm:flex items-center gap-2">
        <span class="text-xs text-muted-foreground">第 {{ currentPage }} / {{ total }} 页</span>
        <span v-if="totalItemsVal !== undefined" class="text-xs text-muted-foreground">｜共 {{ totalItemsVal }} 条</span>
      </div>
    </div>

    <!-- 右侧：分页按钮 -->
    <div class="flex items-center gap-2">
      <!-- 小屏 -->
      <div class="flex sm:hidden items-center gap-1">
        <UiButton size="sm" :disabled="!canPrev" class="h-8 px-2" @click="prev">
          <span class="sr-only">上一页</span>
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M15 19l-7-7 7-7" />
          </svg>
        </UiButton>
        <UiButton size="sm" :disabled="!canNext" class="h-8 px-2" @click="next">
          <span class="sr-only">下一页</span>
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 5l7 7-7 7" />
          </svg>
        </UiButton>
      </div>

      <!-- 中屏及以上 -->
      <div class="hidden sm:flex items-center gap-1 rounded-md border bg-background p-1 shadow-sm">
        <!-- 首页 -->
        <UiButton size="sm" :disabled="!canPrev" class="h-8 w-8 p-0" @click="first" aria-label="首页">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M13 17l-5-5 5-5" />
            <path d="M19 17l-5-5 5-5" />
          </svg>
        </UiButton>
        <!-- 上一页 -->
        <UiButton size="sm" :disabled="!canPrev" class="h-8 w-8 p-0" @click="prev" aria-label="上一页">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M15 19l-7-7 7-7" />
          </svg>
        </UiButton>

        <!-- 左省略 -->
        <template v-if="showFirst">
          <UiButton size="sm" variant="ghost" class="h-8 min-w-[2rem] px-2" @click="setPage(1)">1</UiButton>
          <span class="px-1 text-muted-foreground">…</span>
        </template>

        <!-- 页码 -->
        <template v-for="p in pages" :key="p">
          <UiButton
            size="sm"
            class="h-8 min-w-[2rem] px-2"
            :variant="p === currentPage ? 'default' : 'ghost'"
            @click="setPage(p)"
          >
            {{ p }}
          </UiButton>
        </template>

        <!-- 右省略 -->
        <template v-if="showLast">
          <span class="px-1 text-muted-foreground">…</span>
          <UiButton size="sm" variant="ghost" class="h-8 min-w-[2rem] px-2" @click="setPage(total)">{{ total }}</UiButton>
        </template>

        <!-- 下一页 -->
        <UiButton size="sm" :disabled="!canNext" class="h-8 w-8 p-0" @click="next" aria-label="下一页">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 5l7 7-7 7" />
          </svg>
        </UiButton>
        <!-- 末页 -->
        <UiButton size="sm" :disabled="!canNext" class="h-8 w-8 p-0" @click="last" aria-label="末页">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M11 7l5 5-5 5" />
            <path d="M5 7l5 5-5 5" />
          </svg>
        </UiButton>
      </div>

      <!-- 跳页 -->
      <div class="hidden md:flex items-center gap-1 ml-2">
        <UiInput
          type="number"
          :class="'h-8 w-16 px-2 text-xs'"
          v-model="jumpInput"
          placeholder="跳页" />
        <UiButton size="sm" @click="jump">跳转</UiButton>
      </div>
    </div>
  </div>
</template>
