<script setup lang="ts">
defineOptions({ name: 'AdminAudit' })
import { computed, ref } from 'vue'
import { ChevronDown, Copy, RefreshCw, ScrollText, Search } from '@lucide/vue'
import { listAudit } from '../api'
import type { Audit } from '../api/type'
import { useCursorPage } from '@/composables/use-cursor-page'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
const list = useCursorPage('identity-audit', listAudit),
    { data, status, error, refreshing } = list
const query = ref(''),
    resultFilter = ref<'all' | 'success' | 'other'>('all'),
    expanded = ref<string[]>([])
const normalizedResult = (event: Audit) => event.result.toLowerCase()
const resultTone = (event: Audit) => {
    const result = normalizedResult(event)
    if (['success', 'ok', 'allowed', 'accepted'].some((value) => result.includes(value)))
        return 'status-success'
    if (['denied', 'rejected', 'failure', 'failed', 'error'].some((value) => result.includes(value)))
        return 'status-danger'
    return 'status-muted'
}
const resultLabel = (event: Audit) => {
    const result = normalizedResult(event)
    if (['success', 'ok', 'allowed', 'accepted'].some((value) => result.includes(value))) return '成功'
    if (['denied', 'rejected'].some((value) => result.includes(value))) return '拒绝'
    if (['failure', 'failed', 'error'].some((value) => result.includes(value))) return '失败'
    return event.result
}
const filtered = computed(() => {
    const keyword = query.value.trim().toLowerCase()
    return (data.value?.items ?? []).filter((event) => {
        const matchesResult =
            resultFilter.value === 'all' ||
            (resultFilter.value === 'success'
                ? resultTone(event) === 'status-success'
                : resultTone(event) !== 'status-success')
        const matchesQuery =
            !keyword ||
            event.action.toLowerCase().includes(keyword) ||
            event.object_kind.toLowerCase().includes(keyword) ||
            event.object_id.toLowerCase().includes(keyword) ||
            String(event.actor_id ?? 'anonymous').includes(keyword)
        return matchesResult && matchesQuery
    })
})
function toggle(id: string) {
    expanded.value = expanded.value.includes(id)
        ? expanded.value.filter((value) => value !== id)
        : [...expanded.value, id]
}
async function copy(value: string, label: string) {
    try {
        await navigator.clipboard.writeText(value)
        emitToast({ type: 'success', message: label + '已复制' })
    } catch {
        emitToast({ type: 'error', message: '无法访问剪贴板，请手动复制' })
    }
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="身份审计"
            eyebrow="AUDIT"
            description="查询已提交的账户、应用与授权操作摘要。记录只读，变更会写入不可篡改的审计链。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.first()">
                <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />重新读取
            </UiButton>
        </PageHeader>

        <section class="panel">
            <div class="table-toolbar">
                <div class="search-box">
                    <Search :size="15" aria-hidden="true" />
                    <UiInput v-model="query" clearable placeholder="筛选本页：动作、对象或操作者…" aria-label="筛选审计记录" />
                </div>
                <div class="filter-chips" role="group" aria-label="结果筛选">
                    <button class="filter-chip" type="button" :aria-pressed="resultFilter === 'all'" @click="resultFilter = 'all'">全部</button>
                    <button class="filter-chip" type="button" :aria-pressed="resultFilter === 'success'" @click="resultFilter = 'success'">成功</button>
                    <button class="filter-chip" type="button" :aria-pressed="resultFilter === 'other'" @click="resultFilter = 'other'">拒绝 / 失败</button>
                </div>
                <span class="toolbar-spacer inline-flex items-center gap-1.5 text-[11px] text-muted-foreground">
                    <ScrollText :size="13" aria-hidden="true" />仅筛选当前已加载记录
                </span>
            </div>

            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!data?.items.length"
                empty-title="暂无审计记录"
                empty-description="账户、应用与授权操作提交后，会在这里生成摘要。"
                :skeleton-rows="4"
                @retry="list.refresh()"
            >
                <div v-if="filtered.length" class="divide-y divide-border">
                    <article v-for="event in filtered" :key="event.id" class="px-5 py-4 transition-colors hover:bg-accent/20">
                        <div class="flex flex-wrap items-start justify-between gap-3">
                            <div class="min-w-0">
                                <div class="flex flex-wrap items-center gap-2">
                                    <h2 class="text-[13px] font-semibold">{{ event.action }}</h2>
                                    <span class="status-badge" :class="resultTone(event)">{{ resultLabel(event) }}</span>
                                    <span v-if="event.reason_code" class="status-badge status-muted">{{ event.reason_code }}</span>
                                </div>
                                <p class="mt-1.5 text-[11px] text-muted-foreground">
                                    {{ new Date(event.happened_at * 1000).toLocaleString('zh-CN') }}
                                    · 操作者 {{ event.actor_id ?? '匿名' }}
                                    · {{ event.object_kind }} {{ event.object_id }}
                                </p>
                            </div>
                            <div class="flex items-center gap-1">
                                <button
                                    type="button"
                                    class="icon-button !h-7 !w-7"
                                    aria-label="复制请求 ID"
                                    title="复制请求 ID"
                                    @click="copy(event.request_id, '请求 ID')"
                                >
                                    <Copy :size="12" aria-hidden="true" />
                                </button>
                                <UiButton
                                    v-if="Object.keys(event.change_summary).length"
                                    variant="ghost"
                                    size="sm"
                                    :aria-expanded="expanded.includes(event.id)"
                                    @click="toggle(event.id)"
                                >
                                    {{ expanded.includes(event.id) ? '收起详情' : '查看详情' }}
                                    <ChevronDown
                                        :size="13"
                                        class="transition-transform"
                                        :class="expanded.includes(event.id) && 'rotate-180'"
                                        aria-hidden="true"
                                    />
                                </UiButton>
                            </div>
                        </div>
                        <div
                            v-if="expanded.includes(event.id)"
                            class="mt-3 rounded-lg border border-border bg-muted/30 p-3"
                        >
                            <dl class="grid gap-2 text-[11px] sm:grid-cols-2">
                                <div v-for="(value, key) in event.change_summary" :key="key">
                                    <dt class="font-semibold text-muted-foreground">{{ key }}</dt>
                                    <dd class="mt-0.5 break-all text-foreground">{{ value }}</dd>
                                </div>
                            </dl>
                            <p class="mt-2 select-all font-mono text-[10px] text-muted-foreground">请求 {{ event.request_id }}</p>
                        </div>
                    </article>
                </div>
                <p v-else class="empty-filter">没有匹配当前筛选条件的审计记录。</p>
                <div v-if="data?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多审计记录</UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
