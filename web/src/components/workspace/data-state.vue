<script setup lang="ts">
import { CircleAlert, SearchX, RefreshCw } from '@lucide/vue'
import UiButton from '@/components/ui/button/button.vue'
defineProps<{
    status: string
    empty?: boolean
    error?: string | null
    refreshing?: boolean
    emptyTitle?: string
    emptyDescription?: string
    skeletonRows?: number
}>()
defineEmits<{ retry: [] }>()
</script>
<template>
    <div v-if="status === 'loading' || status === 'idle'" aria-busy="true">
        <div class="skeleton-stack" aria-hidden="true">
            <div v-for="row in skeletonRows ?? 3" :key="row" class="skeleton-row">
                <span class="skeleton skeleton-avatar" />
                <span class="flex-1 space-y-2">
                    <span class="skeleton skeleton-line block w-1/3" />
                    <span class="skeleton skeleton-line block w-2/3" />
                </span>
            </div>
        </div>
        <p class="sr-only" role="status">正在加载，请稍候。</p>
    </div>
    <div v-else-if="status === 'error'" class="data-state" role="alert">
        <span class="data-state-icon is-error"><CircleAlert :size="22" aria-hidden="true" /></span>
        <h2>加载失败</h2>
        <p>{{ error || '暂时无法获取数据，请稍后重试。' }}</p>
        <div class="data-state-actions">
            <UiButton variant="outline" @click="$emit('retry')">
                <RefreshCw :size="14" aria-hidden="true" />重新加载
            </UiButton>
        </div>
    </div>
    <template v-else>
        <div v-if="refreshing" class="refresh-bar" role="status" aria-label="正在刷新" />
        <p v-if="error" class="inline-error m-4" role="alert">
            刷新失败，以下为上次结果。{{ error }}
            <UiButton variant="ghost" size="sm" @click="$emit('retry')">重试</UiButton>
        </p>
        <div v-if="empty" class="data-state">
            <span class="data-state-icon"><SearchX :size="22" aria-hidden="true" /></span>
            <h2>{{ emptyTitle || '暂无数据' }}</h2>
            <p>{{ emptyDescription || '调整筛选条件，或添加第一条记录。' }}</p>
            <div v-if="$slots.actions" class="data-state-actions"><slot name="actions" /></div>
        </div>
        <slot v-else />
    </template>
</template>
