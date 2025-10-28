<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'

const props = defineProps<{
  pageNumber: number
  totalPages: number
  pageSize?: number
  pageSizeOptions?: number[]
  totalItems?: number
  disabled?: boolean
}>()

const emits = defineEmits<{
  (e: 'update:pageNumber', value: number): void
  (e: 'change', value: number): void
  (e: 'update:pageSize', value: number): void
  (e: 'size-change', value: number): void
}>()

// ========== 计算属性 ==========
const total = computed(() => Math.max(1, props.totalPages))
const canPrev = computed(() => props.pageNumber > 1)
const canNext = computed(() => props.pageNumber < total.value)
const sizeOptions = computed(() => props.pageSizeOptions ?? [10, 20, 50])

// ========== 分页跳转逻辑 ==========
function setPage(p: number) {
  if (props.disabled) return
  const next = Math.min(Math.max(1, p), total.value)
  if (next !== props.pageNumber) {
    emits('update:pageNumber', next)
    emits('change', next)
  }
}

const prev = () => canPrev.value && setPage(props.pageNumber - 1)
const next = () => canNext.value && setPage(props.pageNumber + 1)
const first = () => setPage(1)
const last = () => setPage(total.value)

// ========== 页码范围（半径可调） ==========
const pages = computed(() => {
  const radius = 2
  const start = Math.max(1, props.pageNumber - radius)
  const end = Math.min(total.value, props.pageNumber + radius)
  return Array.from({ length: end - start + 1 }, (_, i) => start + i)
})

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
  emits('update:pageSize', size)
  emits('size-change', size)
}
</script>

<template>
  <div class="flex flex-wrap items-center justify-between gap-2 mt-3 select-none">
    <!-- 左侧：页容量与信息 -->
    <div class="flex items-center gap-2">
      <div v-if="props.pageSize" class="flex items-center gap-1">
      <span class="text-xs text-muted-foreground">每页</span>
        <UiSelect :model-value="props.pageSize" class="h-8 px-2 text-xs w-20" @update:modelValue="(v:any) => setSize(Number(v))">
          <option v-for="opt in sizeOptions" :key="opt" :value="opt">{{ opt }}</option>
        </UiSelect>
      <span class="text-xs text-muted-foreground">条</span>
      </div>
    <span class="text-xs text-muted-foreground">
        第 {{ props.pageNumber }} / {{ total }} 页
      </span>
    <span v-if="props.totalItems" class="text-xs text-muted-foreground">
        ｜共 {{ props.totalItems }} 条
      </span>
    </div>

    <!-- 右侧：分页按钮 -->
    <div class="flex items-center gap-1">
      <!-- 小屏 -->
      <div class="flex sm:hidden items-center gap-1">
        <UiButton size="sm" :disabled="!canPrev" @click="prev">上一页</UiButton>
        <UiButton size="sm" :disabled="!canNext" @click="next">下一页</UiButton>
      </div>

      <!-- 中屏及以上 -->
      <div class="hidden sm:flex items-center gap-1">
        <div class="flex items-center gap-1">
          <UiButton size="sm" :disabled="!canPrev" @click="first">首页</UiButton>
          <UiButton size="sm" :disabled="!canPrev" @click="prev">上一页</UiButton>
        </div>

        <!-- 页码 -->
        <div class="flex items-center gap-1 sm:gap-1.5">
          <UiButton
            v-for="p in pages"
            :key="p"
            size="sm"
            :variant="p === props.pageNumber ? 'default' : 'ghost'"
            @click="setPage(p)"
          >
            {{ p }}
          </UiButton>
        </div>

        <div class="flex items-center gap-1">
          <UiButton size="sm" :disabled="!canNext" @click="next">下一页</UiButton>
          <UiButton size="sm" :disabled="!canNext" @click="last">末页</UiButton>
        </div>
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
