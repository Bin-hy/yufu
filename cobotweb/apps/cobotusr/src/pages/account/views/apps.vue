<script setup lang="ts">
defineOptions({ name: "AccountApps" })
import { getApplications } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const state = useCursorPage('personal-apps', getApplications), { data, status, error, refreshing } = state
</script>
<template>
    <div class="workspace-page">
        <PageHeader title="可访问应用" description="企业已明确分配给你的应用环境；应用维护关系仅提供开发环境准入。"><UiButton variant="outline" @click="state.first">刷新</UiButton></PageHeader>
        <DataState :status="status" :error="error" :refreshing="refreshing" :empty="!data?.items.length" empty-description="联系企业管理员分配应用访问权。" @retry="state.refresh">
            <div class="grid gap-4 md:grid-cols-2"><article v-for="app in data?.items" :key="app.environment_id" class="rounded-xl border bg-card p-5"><h2 class="font-semibold">{{ app.name }}</h2><p class="mt-2 text-sm text-muted-foreground">{{ app.environment === 'development' ? '开发环境' : '生产环境' }}</p><p class="mt-2 text-xs text-muted-foreground">从应用自己的登录入口选择企业统一登录。</p></article></div>
        </DataState>
        <UiButton v-if="data?.next" class="mt-4" variant="outline" @click="state.next">下一页</UiButton>
    </div>
</template>
