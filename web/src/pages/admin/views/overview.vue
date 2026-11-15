<script setup lang="ts">
defineOptions({ name: 'AdminOverview' })
import { computed } from 'vue'
import {
    AppWindow,
    Network,
    RefreshCw,
    ScrollText,
    ShieldCheck,
    TriangleAlert,
    Users,
} from '@lucide/vue'
import { getOverview } from '../api'
import { useAsyncQuery } from '@/composables/use-async-query'
import { usePageContext } from '@/workspace/page-context'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'

const context = usePageContext()
const overview = useAsyncQuery({
    key: 'admin-overview',
    active: () => context.active.value,
    loader: (signal) => getOverview(signal),
})
const { data, status, error, refreshing } = overview
const attention = computed(() => {
    if (!data.value) return []
    const entries: { label: string; count: number; to: { name: string; query?: Record<string, string> } }[] = []
    if (data.value.approvals && data.value.approvals.pending > 0)
        entries.push({ label: '待审批的生产配置', count: data.value.approvals.pending, to: { name: 'admin-apps' } })
    if (data.value.deliveries) {
        const failed = data.value.deliveries.permanent_failure + data.value.deliveries.retry
        if (failed > 0)
            entries.push({ label: '失败或待重试的退出通知', count: failed, to: { name: 'admin-deliveries' } })
    }
    if (data.value.migrations && data.value.migrations.unmanaged > 0)
        entries.push({ label: '待纳管的历史客户端', count: data.value.migrations.unmanaged, to: { name: 'admin-migrations' } })
    if (data.value.audit && data.value.audit.recent_failures > 0)
        entries.push({ label: '近 24 小时非成功审计事件', count: data.value.audit.recent_failures, to: { name: 'admin-audit' } })
    return entries
})
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="管理总览"
            eyebrow="OVERVIEW"
            description="企业身份治理的当前状态与需要处理的事项。各区块按你的读取权限展示。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="overview.refresh()">
                <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />重新读取
            </UiButton>
        </PageHeader>

        <DataState
            :status="status"
            :error="error"
            :refreshing="refreshing"
            :empty="false"
            :skeleton-rows="3"
            @retry="overview.refresh()"
        >
            <section v-if="attention.length" class="panel mb-5 border-warning/30">
                <header class="panel-heading bg-warning/[0.04]">
                    <div>
                        <h2 class="flex items-center gap-2">
                            <TriangleAlert :size="15" class="text-warning" aria-hidden="true" />今天需要处理
                        </h2>
                        <p>以下事项等待管理员跟进。</p>
                    </div>
                </header>
                <ul class="divide-y divide-border">
                    <li v-for="entry in attention" :key="entry.label">
                        <RouterLink
                            :to="entry.to"
                            class="flex items-center justify-between gap-3 px-5 py-3 text-xs transition-colors hover:bg-accent/20"
                        >
                            <span>{{ entry.label }}</span>
                            <span class="status-badge status-warning">{{ entry.count }}</span>
                        </RouterLink>
                    </li>
                </ul>
            </section>
            <p v-else class="inline-notice mb-5">
                <ShieldCheck :size="15" class="mt-0.5 shrink-0 text-success" aria-hidden="true" />
                没有待处理事项：审批、投递、纳管与近期审计均无异常。
            </p>

            <div class="metric-grid">
                <RouterLink v-if="data?.accounts" class="metric-card transition-colors hover:border-primary/40" :to="{ name: 'admin-accounts' }">
                    <span class="metric-label"><Users :size="13" aria-hidden="true" />企业账户</span>
                    <strong class="metric-value">{{ data.accounts.total }}</strong>
                    <span class="metric-meta">停用 {{ data.accounts.disabled }} · 开发者 {{ data.accounts.developers }}</span>
                </RouterLink>
                <RouterLink v-if="data?.approvals" class="metric-card transition-colors hover:border-primary/40" :to="{ name: 'admin-apps' }">
                    <span class="metric-label"><AppWindow :size="13" aria-hidden="true" />待审批</span>
                    <strong class="metric-value" :class="data.approvals.pending ? '!text-warning' : ''">{{ data.approvals.pending }}</strong>
                    <span class="metric-meta">等待管理员审批的生产配置</span>
                </RouterLink>
                <RouterLink v-if="data?.deliveries" class="metric-card transition-colors hover:border-primary/40" :to="{ name: 'admin-deliveries' }">
                    <span class="metric-label"><Network :size="13" aria-hidden="true" />退出通知</span>
                    <strong class="metric-value" :class="data.deliveries.permanent_failure + data.deliveries.retry ? '!text-destructive' : ''">
                        {{ data.deliveries.permanent_failure + data.deliveries.retry }}
                    </strong>
                    <span class="metric-meta">失败 {{ data.deliveries.permanent_failure }} · 待重试 {{ data.deliveries.retry }} · 已送达 {{ data.deliveries.delivered }}</span>
                </RouterLink>
                <RouterLink v-if="data?.migrations" class="metric-card transition-colors hover:border-primary/40" :to="{ name: 'admin-migrations' }">
                    <span class="metric-label"><ShieldCheck :size="13" aria-hidden="true" />历史接入</span>
                    <strong class="metric-value" :class="data.migrations.unmanaged ? '!text-warning' : ''">{{ data.migrations.unmanaged }}</strong>
                    <span class="metric-meta">待纳管客户端 · 未解决冲突 {{ data.migrations.unresolved_conflicts }}</span>
                </RouterLink>
                <RouterLink v-if="data?.audit" class="metric-card transition-colors hover:border-primary/40" :to="{ name: 'admin-audit' }">
                    <span class="metric-label"><ScrollText :size="13" aria-hidden="true" />近 24 小时审计</span>
                    <strong class="metric-value" :class="data.audit.recent_failures ? '!text-destructive' : ''">{{ data.audit.recent_failures }}</strong>
                    <span class="metric-meta">拒绝或失败的操作事件</span>
                </RouterLink>
            </div>
        </DataState>
    </div>
</template>
