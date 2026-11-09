<script setup lang="ts">
defineOptions({ name: "AdminDeliveries" })
import { listDeliveries, retryDelivery } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const list = useCursorPage('logout-deliveries', listDeliveries), { data, status, error } = list
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation
const statusNames: Record<string, string> = { pending: '等待发送', leased: '发送处理中', retry: '等待重试', delivered: '已送达', permanent_failure: '发送失败', expired: '已超过期限' }
async function retry(id: string) { await mutation.run(() => retryDelivery(id), () => list.refresh()) }
</script>
<template>
    <div class="workspace-page"><PageHeader title="退出通知" description="通知结果用于运维追踪，授权撤销在通知发送前已生效。"><UiButton variant="outline" @click="list.first">重新读取</UiButton></PageHeader><p v-if="writeError" class="inline-error mb-4" role="alert">{{ writeError }}</p><DataState :status="status" :error="error" :empty="!data?.items.length" @retry="list.refresh"><div class="space-y-3"><article v-for="delivery in data?.items" :key="delivery.id" class="flex flex-wrap items-center justify-between gap-4 rounded-xl border bg-card p-5"><div><h2 class="font-semibold">{{ delivery.name }} · {{ delivery.environment === 'development' ? '开发' : '生产' }}</h2><p class="mt-2 text-xs">{{ statusNames[delivery.status] || delivery.status }} · 尝试 {{ delivery.attempt_count }} 次{{ delivery.last_error_code ? ' · ' + delivery.last_error_code : '' }}</p><p class="mt-2 text-xs text-muted-foreground">建立 {{ new Date(delivery.created_at * 1000).toLocaleString() }}{{ delivery.delivered_at ? ' · 送达 ' + new Date(delivery.delivered_at * 1000).toLocaleString() : '' }}</p></div><UiButton v-if="!['delivered', 'expired', 'leased'].includes(delivery.status)" variant="outline" :disabled="pending" @click="retry(delivery.id)">按原目标重新排队</UiButton></article></div></DataState><UiButton v-if="data?.next" class="mt-4" variant="outline" @click="list.next">下一页</UiButton></div>
</template>
