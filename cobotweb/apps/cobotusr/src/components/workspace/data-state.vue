<script setup lang="ts">
import { LoaderCircle, SearchX, CircleAlert } from '@lucide/vue'
import UiButton from '@/components/ui/button/button.vue'
defineProps<{
    status: string
    empty?: boolean
    error?: string | null
    refreshing?: boolean
    emptyTitle?: string
    emptyDescription?: string
}>()
defineEmits<{ retry: [] }>()
</script>
<template>
    <div v-if="status === 'loading' || status === 'idle'" class="data-state" role="status">
        <LoaderCircle
            :size="24"
            class="mx-auto mb-3 animate-spin text-primary"
            aria-hidden="true"
        />
        <h2>正在加载</h2>
        <p>请稍候，数据即将显示。</p>
    </div>
    <div v-else-if="status === 'error'" class="data-state" role="alert">
        <CircleAlert :size="24" class="mx-auto mb-3 text-destructive" aria-hidden="true" />
        <h2>加载失败</h2>
        <p>{{ error || '暂时无法获取数据' }}</p>
        <UiButton class="mt-4" variant="outline" @click="$emit('retry')">重新加载</UiButton>
    </div>
    <template v-else
        ><p v-if="error" class="inline-error m-4" role="alert">
            刷新失败，以下为上次结果。{{ error }}
            <UiButton variant="ghost" size="sm" @click="$emit('retry')">重试</UiButton>
        </p>
        <p v-if="refreshing" class="px-5 py-2 text-xs text-muted-foreground" role="status">
            正在刷新…
        </p>
        <div v-if="empty" class="data-state">
            <SearchX :size="24" class="mx-auto mb-3" aria-hidden="true" />
            <h2>{{ emptyTitle || '暂无数据' }}</h2>
            <p>{{ emptyDescription || '调整筛选条件，或添加第一条记录。' }}</p>
        </div>
        <slot v-else
    /></template>
</template>
