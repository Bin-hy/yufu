<script setup lang="ts">
defineOptions({ name: "AdminAudit" })
import { listAudit } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const list = useCursorPage('identity-audit', listAudit), { data, status, error } = list
</script>
<template>
    <div class="workspace-page"><PageHeader title="身份审计" description="查询已提交的账户、应用与授权操作摘要。"><UiButton variant="outline" @click="list.first">重新读取</UiButton></PageHeader><DataState :status="status" :error="error" :empty="!data?.items.length" @retry="list.refresh"><div class="space-y-3"><article v-for="event in data?.items" :key="event.id" class="rounded-xl border bg-card p-5"><h2 class="font-semibold">{{ event.action }} · {{ event.result }}</h2><p class="mt-2 text-xs text-muted-foreground">{{ new Date(event.happened_at * 1000).toLocaleString() }} · 操作者 {{ event.actor_id ?? '匿名' }} · {{ event.object_kind }} {{ event.object_id }}</p><p class="mt-2 select-all text-xs">请求 {{ event.request_id }}{{ event.reason_code ? ' · ' + event.reason_code : '' }}</p><dl class="mt-3 grid gap-2 text-xs sm:grid-cols-2"><div v-for="(value, key) in event.change_summary" :key="key"><dt class="text-muted-foreground">{{ key }}</dt><dd class="mt-1 break-all">{{ value }}</dd></div></dl></article></div></DataState><UiButton v-if="data?.next" class="mt-4" variant="outline" @click="list.next">下一页</UiButton></div>
</template>
