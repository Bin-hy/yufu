<script setup lang="ts">
defineOptions({ name: "AccountConsents" })
import { getConsents, revokeConsent } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const state = useCursorPage('personal-consents', getConsents), { data, status, error } = state
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation
async function withdraw(id: number) { await mutation.run(() => revokeConsent(id), async () => { await state.refresh(); emitToast({ type: 'success', message: '资料授权已撤回，该环境原应用授权已失效' }) }) }
const scopeNames: Record<string, string> = { openid: '账户身份', profile: '用户名与显示名称', email: '邮箱与验证状态' }
</script>
<template>
    <div class="workspace-page"><PageHeader title="资料授权" description="撤回后该应用环境需要重新获取你的同意；企业访问分配保持独立管理。" />
        <p v-if="writeError" role="alert" class="inline-error mb-4">{{ writeError }}</p>
        <DataState :status="status" :error="error" :empty="!data?.items.length" empty-description="尚未同意应用读取账户资料。" @retry="state.refresh">
            <div class="space-y-3"><article v-for="consent in data?.items" :key="consent.id" class="flex flex-wrap items-center justify-between gap-4 rounded-xl border bg-card p-5"><div><h2 class="font-semibold">{{ consent.name }} · {{ consent.environment === 'development' ? '开发' : '生产' }}</h2><p class="mt-2 text-sm text-muted-foreground">{{ consent.scopes.map(s => scopeNames[s] || s).join('、') }}</p><p v-if="!consent.current_version" class="mt-2 text-xs">对应历史配置版本</p></div><UiButton variant="outline" :disabled="pending" @click="withdraw(consent.id)">撤回授权</UiButton></article></div>
        </DataState><UiButton v-if="data?.next" class="mt-4" variant="outline" @click="state.next">下一页</UiButton>
    </div>
</template>
