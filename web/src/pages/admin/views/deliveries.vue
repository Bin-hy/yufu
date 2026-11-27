<script setup lang="ts">
defineOptions({ name: 'AdminDeliveries' })
import { computed, onBeforeUnmount } from 'vue'
import { RefreshCw, RotateCcw, Send, TriangleAlert } from '@lucide/vue'
import { listDeliveries, retryDelivery, getOverview } from '../api'
import { useCursorList } from '@/composables/use-cursor-list'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useRowMutation } from '@/composables/use-row-mutation'
import { useRouteFilters } from '@/composables/use-route-filters'
import { usePageContext } from '@/workspace/page-context'
import { emitToast } from '@/utils/toast'
import {
    deliveryRetryable,
    deliveryStatusName,
    deliveryStatusTone,
    deliveryStatuses,
    environmentName,
} from '@/lib/identity-labels'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import RelativeTime from '@/components/workspace/relative-time.vue'
import UiButton from '@/components/ui/button/button.vue'

const context = usePageContext()
const filters = useRouteFilters({ status: 'all' })
const statusFilter = filters.model('status')
const list = useCursorList(
    () => `logout-deliveries:${filters.values.value.status}`,
    (after, signal) =>
        listDeliveries(
            after,
            signal,
            filters.values.value.status === 'all' ? undefined : filters.values.value.status,
        ),
),
    { items, hasMore, status, error, refreshing } = list
const overview = useAsyncQuery({
    key: 'admin-overview:deliveries',
    active: () => context.active.value,
    loader: (signal) => getOverview(signal),
})
const stats = computed(() => overview.data.value?.deliveries ?? null)
const attention = computed(() =>
    stats.value
        ? stats.value.permanent_failure + stats.value.retry + stats.value.expired
        : null,
)
const inFlight = computed(() =>
    stats.value
        ? stats.value.pending + stats.value.leased + stats.value.retry > 0
        : items.value.some((item) => ['pending', 'leased', 'retry'].includes(item.status)),
)
/** In-flight states change on their own; poll softly while the page is visible. */
const POLL_INTERVAL = 20_000
const timer = setInterval(() => {
    if (!context.active.value || document.hidden || !inFlight.value || refreshing.value) return
    void list.refresh()
    void overview.refresh()
}, POLL_INTERVAL)
onBeforeUnmount(() => clearInterval(timer))
const mutation = useRowMutation(),
    { pending, error: writeError, rowPending } = mutation
async function retry(id: string) {
    await mutation.runRow(
        id,
        () => retryDelivery(id),
        async () => {
            await Promise.all([list.refresh(), overview.refresh()])
            emitToast({ type: 'success', message: '已按原目标重新排队' })
        },
    )
}
function reload() {
    void overview.refresh()
    return list.refresh()
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="退出通知"
            eyebrow="DELIVERIES"
            hint="授权撤销先于通知生效；投递失败不影响本地退出。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="reload">
                <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />刷新
            </UiButton>
        </PageHeader>
        <p v-if="writeError" role="alert" class="form-alert mb-4">{{ writeError }}</p>

        <div v-if="stats" class="metric-grid mb-5">
            <article class="metric-card">
                <span class="metric-label"><Send :size="13" aria-hidden="true" />进行中</span>
                <strong class="metric-value">{{ stats.pending + stats.leased + stats.retry }}</strong>
                <span class="metric-meta">等待 {{ stats.pending }} · 处理中 {{ stats.leased }} · 重试 {{ stats.retry }}</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><TriangleAlert :size="13" aria-hidden="true" />需要关注</span>
                <strong class="metric-value" :class="attention ? '!text-destructive' : ''">{{ attention }}</strong>
                <span class="metric-meta">失败 {{ stats.permanent_failure }} · 重试 {{ stats.retry }} · 过期 {{ stats.expired }}</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><RotateCcw :size="13" aria-hidden="true" />已送达</span>
                <strong class="metric-value">{{ stats.delivered }}</strong>
            </article>
        </div>

        <section class="panel">
            <div class="table-toolbar">
                <div class="filter-chips" role="group" aria-label="投递状态筛选">
                    <button
                        type="button"
                        class="filter-chip"
                        :aria-pressed="statusFilter === 'all'"
                        @click="statusFilter = 'all'"
                    >全部</button>
                    <button
                        v-for="value in deliveryStatuses"
                        :key="value"
                        type="button"
                        class="filter-chip"
                        :aria-pressed="statusFilter === value"
                        @click="statusFilter = value"
                    >
                        {{ deliveryStatusName(value) }}
                    </button>
                </div>
                <span class="toolbar-spacer text-[11px] text-muted-foreground">已加载 {{ items.length }}</span>
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!items.length"
                :empty-title="statusFilter === 'all' ? '暂无退出通知' : '该状态下没有通知'"
                :empty-description="statusFilter === 'all' ? '应用登记退出通知地址后会显示在这里。' : '切换到其他状态或查看全部。'"
                :skeleton-rows="3"
                @retry="list.refresh()"
            >
                <div class="table-scroll desktop-table">
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
                            <tr v-for="delivery in items" :key="delivery.id">
                                <td>
                                    <p class="cell-primary">{{ delivery.name }}</p>
                                    <p class="cell-muted mt-0.5">{{ environmentName(delivery.environment) }}</p>
                                </td>
                                <td>
                                    <span class="status-badge" :class="deliveryStatusTone(delivery.status)">{{ deliveryStatusName(delivery.status) }}</span>
                                    <p v-if="delivery.last_error_code" class="cell-muted mt-1">{{ delivery.last_error_code }}</p>
                                </td>
                                <td>{{ delivery.attempt_count }} 次</td>
                                <td class="cell-muted">
                                    <RelativeTime :at="delivery.created_at" />
                                    <template v-if="delivery.delivered_at"> → <RelativeTime :at="delivery.delivered_at" /></template>
                                </td>
                                <td class="cell-actions">
                                    <UiButton
                                        v-if="deliveryRetryable(delivery.status)"
                                        variant="outline"
                                        size="sm"
                                        :disabled="pending"
                                        :loading="rowPending(delivery.id)"
                                        @click="retry(delivery.id)"
                                    >按原目标重新排队</UiButton>
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="mobile-list">
                    <article v-for="delivery in items" :key="delivery.id" class="list-card">
                        <div class="min-w-0">
                            <div class="flex flex-wrap items-center gap-2">
                                <h3 class="list-title">{{ delivery.name }}</h3>
                                <span class="status-badge" :class="deliveryStatusTone(delivery.status)">{{ deliveryStatusName(delivery.status) }}</span>
                            </div>
                            <p class="list-meta">{{ environmentName(delivery.environment) }} · 尝试 {{ delivery.attempt_count }} 次{{ delivery.last_error_code ? ' · ' + delivery.last_error_code : '' }}</p>
                            <p class="list-meta">建立 <RelativeTime :at="delivery.created_at" /></p>
                        </div>
                        <UiButton
                            v-if="deliveryRetryable(delivery.status)"
                            variant="outline"
                            size="sm"
                            :disabled="pending"
                            :loading="rowPending(delivery.id)"
                            @click="retry(delivery.id)"
                        >按原目标重新排队</UiButton>
                    </article>
                </div>
                <div v-if="hasMore" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多通知</UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
