<script setup lang="ts">
defineOptions({ name: 'AdminDeliveries' })
import { computed, ref } from 'vue'
import { RefreshCw, RotateCcw, Send, TriangleAlert } from '@lucide/vue'
import { listDeliveries, retryDelivery } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const list = useCursorPage('logout-deliveries', listDeliveries),
    { data, status, error, refreshing } = list
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation
const statusFilter = ref('all')
const statusNames: Record<string, string> = {
    pending: '等待发送',
    leased: '发送处理中',
    retry: '等待重试',
    delivered: '已送达',
    permanent_failure: '发送失败',
    expired: '已超过期限',
}
const statusTone = (status: string) =>
    status === 'delivered'
        ? 'status-success'
        : ['pending', 'leased', 'retry'].includes(status)
          ? 'status-info'
          : status === 'expired'
            ? 'status-muted'
            : 'status-danger'
const statuses = computed(() => {
    const values = new Set((data.value?.items ?? []).map((item) => item.status))
    return ['all', ...values]
})
const filtered = computed(() =>
    statusFilter.value === 'all'
        ? (data.value?.items ?? [])
        : (data.value?.items ?? []).filter((item) => item.status === statusFilter.value),
)
const failureCount = computed(
    () =>
        data.value?.items.filter((item) =>
            ['permanent_failure', 'retry', 'expired'].includes(item.status),
        ).length ?? 0,
)
async function retry(id: string) {
    await mutation.run(
        () => retryDelivery(id),
        async () => {
            await list.refresh()
            emitToast({ type: 'success', message: '已按原目标重新排队' })
        },
    )
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="退出通知"
            eyebrow="DELIVERIES"
            description="通知结果用于运维追踪。授权撤销在通知发送前已经生效，投递失败不会影响本地退出。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.first()">
                <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />重新读取
            </UiButton>
        </PageHeader>
        <p v-if="writeError" role="alert" class="form-alert mb-4">{{ writeError }}</p>

        <div class="metric-grid mb-5">
            <article class="metric-card">
                <span class="metric-label"><Send :size="13" aria-hidden="true" />当前页记录</span>
                <strong class="metric-value">{{ data?.items.length ?? 0 }}</strong>
                <span class="metric-meta">已加载的退出通知</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><TriangleAlert :size="13" aria-hidden="true" />需要关注</span>
                <strong class="metric-value">{{ failureCount }}</strong>
                <span class="metric-meta">失败、待重试或已过期</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><RotateCcw :size="13" aria-hidden="true" />重试策略</span>
                <strong class="metric-value !text-base">按原目标排队</strong>
                <span class="metric-meta">重试不会撤销已生效的本地授权</span>
            </article>
        </div>

        <section class="panel">
            <div class="table-toolbar">
                <div class="filter-chips" role="group" aria-label="投递状态筛选">
                    <button
                        v-for="value in statuses"
                        :key="value"
                        type="button"
                        class="filter-chip"
                        :aria-pressed="statusFilter === value"
                        @click="statusFilter = value"
                    >
                        {{ value === 'all' ? '全部' : statusNames[value] || value }}
                    </button>
                </div>
                <span class="toolbar-spacer text-[11px] text-muted-foreground">{{ filtered.length }} 条通知</span>
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!data?.items.length"
                empty-title="暂无退出通知"
                empty-description="应用登记退出通知地址后，投递结果会显示在这里。"
                :skeleton-rows="3"
                @retry="list.refresh()"
            >
                <div v-if="filtered.length" class="table-scroll desktop-table">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>应用</th>
                                <th>状态</th>
                                <th>尝试</th>
                                <th>建立 / 送达</th>
                                <th class="text-right">操作</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="delivery in filtered" :key="delivery.id">
                                <td>
                                    <p class="cell-primary">{{ delivery.name }}</p>
                                    <p class="cell-muted mt-0.5">{{ delivery.environment === 'development' ? '开发环境' : '生产环境' }}</p>
                                </td>
                                <td>
                                    <span class="status-badge" :class="statusTone(delivery.status)">{{ statusNames[delivery.status] || delivery.status }}</span>
                                    <p v-if="delivery.last_error_code" class="cell-muted mt-1">{{ delivery.last_error_code }}</p>
                                </td>
                                <td>{{ delivery.attempt_count }} 次</td>
                                <td class="cell-muted">
                                    {{ new Date(delivery.created_at * 1000).toLocaleString('zh-CN') }}
                                    <span v-if="delivery.delivered_at"> → {{ new Date(delivery.delivered_at * 1000).toLocaleString('zh-CN') }}</span>
                                </td>
                                <td class="cell-actions">
                                    <UiButton
                                        v-if="!['delivered', 'expired', 'leased'].includes(delivery.status)"
                                        variant="outline"
                                        size="sm"
                                        :disabled="pending"
                                        :loading="pending"
                                        @click="retry(delivery.id)"
                                    >按原目标重新排队</UiButton>
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="mobile-list">
                    <article v-for="delivery in filtered" :key="delivery.id" class="list-card">
                        <div class="min-w-0">
                            <div class="flex flex-wrap items-center gap-2">
                                <h3 class="list-title">{{ delivery.name }}</h3>
                                <span class="status-badge" :class="statusTone(delivery.status)">{{ statusNames[delivery.status] || delivery.status }}</span>
                            </div>
                            <p class="list-meta">{{ delivery.environment === 'development' ? '开发' : '生产' }} · 尝试 {{ delivery.attempt_count }} 次{{ delivery.last_error_code ? ' · ' + delivery.last_error_code : '' }}</p>
                            <p class="list-meta">建立 {{ new Date(delivery.created_at * 1000).toLocaleString('zh-CN') }}</p>
                        </div>
                        <UiButton
                            v-if="!['delivered', 'expired', 'leased'].includes(delivery.status)"
                            variant="outline"
                            size="sm"
                            :disabled="pending"
                            :loading="pending"
                            @click="retry(delivery.id)"
                        >按原目标重新排队</UiButton>
                    </article>
                </div>
                <p v-if="!filtered.length" class="empty-filter">没有匹配当前筛选条件的通知。</p>
                <div v-if="data?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多通知</UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
