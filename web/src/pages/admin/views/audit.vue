<script setup lang="ts">
defineOptions({ name: 'AdminAudit' })
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ChevronDown, Copy, Download, RefreshCw, Search } from '@lucide/vue'
import { searchAudit } from '../api'
import type { Audit, AuditSearchFilter } from '../api/type'
import { useCursorList } from '@/composables/use-cursor-list'
import { useRouteFilters } from '@/composables/use-route-filters'
import { usePageContext } from '@/workspace/page-context'
import { errorMessage } from '@/composables/use-async-mutation'
import { emitToast } from '@/utils/toast'
import { buildCsv, downloadFile } from '@/utils/csv'
import { auditResultLabel, auditResultTone, auditActionName, auditObjectName } from '@/lib/identity-labels'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import RelativeTime from '@/components/workspace/relative-time.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'

const filters = useRouteFilters({
    result: '',
    from: '',
    to: '',
    actor: '',
    action: '',
    object_kind: '',
})
const resultFilter = filters.model('result'),
    fromFilter = filters.model('from'),
    toFilter = filters.model('to')
const actorInput = ref(filters.values.value.actor),
    actionInput = ref(filters.values.value.action),
    kindInput = ref(filters.values.value.object_kind)
let textTimer: number | undefined
watch([actorInput, actionInput, kindInput], ([actor, action, kind]) => {
    window.clearTimeout(textTimer)
    textTimer = window.setTimeout(() => {
        void filters.set({ actor: actor.trim(), action: action.trim(), object_kind: kind.trim() })
    }, 400)
})
watch(
    () => filters.values.value,
    (values) => {
        if (values.actor !== actorInput.value.trim()) actorInput.value = values.actor
        if (values.action !== actionInput.value.trim()) actionInput.value = values.action
        if (values.object_kind !== kindInput.value.trim()) kindInput.value = values.object_kind
    },
)
const actorValid = computed(
    () => !filters.values.value.actor || /^\d+$/.test(filters.values.value.actor) || filters.values.value.actor === 'anonymous',
)
onBeforeUnmount(() => window.clearTimeout(textTimer))
const rangeValid = computed(() => {
    const { from, to } = filters.values.value
    return !from || !to || new Date(from).getTime() < new Date(to).getTime()
})
function serverFilter(): AuditSearchFilter {
    const values = filters.values.value
    const rfc3339 = (value: string) => (value ? new Date(value).toISOString() : undefined)
    return {
        result: (values.result || undefined) as AuditSearchFilter['result'],
        from: rfc3339(values.from),
        to: rfc3339(values.to),
        actor: actorValid.value && values.actor ? values.actor : undefined,
        action: values.action || undefined,
        object_kind: values.object_kind || undefined,
    }
}
const context = usePageContext()
const list = useCursorList<Audit>(
    () => `identity-audit:${JSON.stringify(filters.values.value)}`,
    (after, signal) => searchAudit(serverFilter(), after, signal),
    () => context.active.value && actorValid.value && rangeValid.value,
),
    { items, hasMore, status, error, refreshing } = list
const expanded = ref<string[]>([])
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
const exporting = ref(false)
const EXPORT_LIMIT = 10_000
async function exportCsv() {
    if (exporting.value) return
    exporting.value = true
    try {
        const rows: Audit[] = []
        let cursor: string | undefined
        const controller = new AbortController()
        do {
            const page = await searchAudit(serverFilter(), cursor, controller.signal, 100)
            rows.push(...page.items)
            cursor = page.next ?? undefined
        } while (cursor && rows.length < EXPORT_LIMIT)
        const truncated = rows.length >= EXPORT_LIMIT && cursor
        const csv = buildCsv(
            ['时间', '动作', '结果', '原因代码', '操作者', '对象类型', '对象', '请求 ID', '变更摘要'],
            rows.slice(0, EXPORT_LIMIT).map((event) => [
                new Date(event.happened_at * 1000).toISOString(),
                event.action,
                event.result,
                event.reason_code,
                event.actor_id ?? 'anonymous',
                event.object_kind,
                event.object_id,
                event.request_id,
                JSON.stringify(event.change_summary),
            ]),
        )
        downloadFile(`audit-${new Date().toISOString().slice(0, 10)}.csv`, csv)
        emitToast({
            type: truncated ? 'warning' : 'success',
            message: truncated
                ? `已导出前 ${EXPORT_LIMIT} 条，更多记录请缩小筛选范围`
                : `已导出 ${rows.length} 条审计记录`,
        })
    } catch (error) {
        emitToast({ type: 'error', message: '导出失败：' + errorMessage(error) })
    } finally {
        exporting.value = false
    }
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="身份审计"
            eyebrow="AUDIT"
            hint="只读检索已提交的账户、应用与授权操作摘要。"
        >
            <UiButton variant="outline" :loading="exporting" :disabled="exporting" @click="exportCsv">
                <Download v-if="!exporting" :size="14" aria-hidden="true" />导出 CSV
            </UiButton>
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.refresh()">
                <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />刷新
            </UiButton>
        </PageHeader>

        <section class="panel">
            <div class="table-toolbar">
                <div class="filter-chips" role="group" aria-label="结果筛选">
                    <button class="filter-chip" type="button" :aria-pressed="resultFilter === ''" @click="resultFilter = ''">全部</button>
                    <button class="filter-chip" type="button" :aria-pressed="resultFilter === 'success'" @click="resultFilter = 'success'">成功</button>
                    <button class="filter-chip" type="button" :aria-pressed="resultFilter === 'denied'" @click="resultFilter = 'denied'">拒绝</button>
                    <button class="filter-chip" type="button" :aria-pressed="resultFilter === 'failure'" @click="resultFilter = 'failure'">失败</button>
                </div>
                <span class="toolbar-spacer inline-flex items-center gap-1.5 text-[11px] text-muted-foreground">
                    <Search :size="13" aria-hidden="true" />服务端筛选 · {{ items.length }}
                </span>
            </div>
            <div class="grid gap-3 border-b border-border px-5 py-4 sm:grid-cols-2 xl:grid-cols-5">
                <UiFormField id="audit-from" label="开始时间" optional>
                    <UiInput id="audit-from" v-model="fromFilter" type="datetime-local" />
                </UiFormField>
                <UiFormField id="audit-to" label="结束时间" optional :hint="rangeValid ? undefined : '结束时间必须晚于开始时间'">
                    <UiInput id="audit-to" v-model="toFilter" type="datetime-local" />
                </UiFormField>
                <UiFormField id="audit-actor" label="操作者" optional :hint="actorValid ? '账户 ID，或 anonymous' : '只能填账户 ID 或 anonymous'">
                    <UiInput id="audit-actor" v-model="actorInput" clearable placeholder="例如 12 或 anonymous" />
                </UiFormField>
                <UiFormField id="audit-action" label="动作（精确匹配）" optional>
                    <UiInput id="audit-action" v-model="actionInput" clearable placeholder="例如 consent.withdraw" />
                </UiFormField>
                <UiFormField id="audit-kind" label="对象类型（精确匹配）" optional>
                    <UiSelect id="audit-kind" v-model="kindInput">
                        <option value="">全部对象</option>
                        <option value="account">account</option>
                        <option value="application">application</option>
                        <option value="group">group</option>
                        <option value="consent">consent</option>
                        <option value="logout_delivery">logout_delivery</option>
                        <option value="invitation">invitation</option>
                        <option value="password_reset">password_reset</option>
                    </UiSelect>
                </UiFormField>
            </div>

            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!items.length"
                empty-title="没有匹配的审计记录"
                empty-description="调整时间或筛选条件后重试。"
                :skeleton-rows="4"
                @retry="list.refresh()"
            >
                <div class="divide-y divide-border">
                    <article v-for="event in items" :key="event.id" class="cv-auto px-5 py-4 transition-colors hover:bg-accent/20">
                        <div class="flex flex-wrap items-start justify-between gap-3">
                            <div class="min-w-0">
                                <div class="flex flex-wrap items-center gap-2">
                                    <h2 class="text-[13px] font-semibold">{{ auditActionName(event.action) }}</h2>
                                    <span class="status-badge" :class="auditResultTone(event.result)">{{ auditResultLabel(event.result) }}</span>
                                    <span v-if="event.reason_code" class="status-badge status-muted">{{ event.reason_code }}</span>
                                </div>
                                <p class="mt-1.5 text-[11px] text-muted-foreground">
                                    <RelativeTime :at="event.happened_at" />
                                    · 操作者 {{ event.actor_id ?? '匿名' }}
                                    · {{ auditObjectName(event.object_kind) }} {{ event.object_id }}
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
                <div v-if="hasMore" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多审计记录</UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
