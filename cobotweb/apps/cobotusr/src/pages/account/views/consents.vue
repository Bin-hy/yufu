<script setup lang="ts">
defineOptions({ name: 'AccountConsents' })
import { computed, ref } from 'vue'
import { KeyRound, Search, ShieldCheck } from '@lucide/vue'
import { getConsents, revokeConsent } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { emitToast } from '@/utils/toast'
import { showModal } from '@/utils/modal'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
const state = useCursorPage('personal-consents', getConsents),
    { data, status, error, refreshing } = state
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation
const query = ref('')
const scopeNames: Record<string, string> = {
    openid: '账户身份',
    profile: '用户名与显示名称',
    email: '邮箱与验证状态',
}
const filtered = computed(() => {
    const keyword = query.value.trim().toLowerCase()
    const items = data.value?.items ?? []
    if (!keyword) return items
    return items.filter(
        (consent) =>
            consent.name.toLowerCase().includes(keyword) ||
            consent.scopes.some((scope) => (scopeNames[scope] || scope).includes(keyword)),
    )
})
async function withdraw(id: number, name: string) {
    const confirmed = await showModal({
        title: '撤回资料授权',
        content: `撤回后，${name} 需要重新获取你的同意才能读取账户资料。企业访问分配不会被改变。`,
        confirmText: '撤回授权',
        destructive: true,
    })
    if (!confirmed) return
    await mutation.run(
        () => revokeConsent(id),
        async () => {
            await state.refresh()
            emitToast({ type: 'success', message: '资料授权已撤回，该环境原应用授权已失效' })
        },
    )
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="资料授权"
            eyebrow="CONSENTS"
            description="撤回后该应用环境需要重新获取你的同意；企业访问分配保持独立管理。"
        />
        <p v-if="writeError" role="alert" class="form-alert mb-4">{{ writeError }}</p>

        <section class="panel">
            <div class="table-toolbar">
                <div class="search-box">
                    <Search :size="15" aria-hidden="true" />
                    <UiInput v-model="query" clearable placeholder="搜索应用或资料范围…" aria-label="搜索资料授权" />
                </div>
                <span class="toolbar-spacer inline-flex items-center gap-1.5 text-[11px] text-muted-foreground">
                    <ShieldCheck :size="13" aria-hidden="true" />
                    授权变更会记录在审计日志
                </span>
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!data?.items.length"
                empty-title="尚未授权任何应用"
                empty-description="当你在应用登录页同意资料访问后，授权记录会显示在这里。"
                :skeleton-rows="2"
                @retry="state.refresh()"
            >
                <div v-if="filtered.length">
                    <article
                        v-for="consent in filtered"
                        :key="consent.id"
                        class="list-row"
                    >
                        <div class="list-row-main">
                            <span class="list-icon"><KeyRound :size="16" aria-hidden="true" /></span>
                            <div class="min-w-0">
                                <div class="flex flex-wrap items-center gap-2">
                                    <h2 class="list-title">{{ consent.name }}</h2>
                                    <span class="status-badge" :class="consent.environment === 'production' ? 'status-success' : 'status-info'">
                                        {{ consent.environment === 'development' ? '开发' : '生产' }}
                                    </span>
                                    <span v-if="!consent.current_version" class="status-badge status-muted">历史配置版本</span>
                                </div>
                                <p class="list-meta">该应用当前可以读取：</p>
                                <div class="mt-2 flex flex-wrap gap-1.5">
                                    <span v-for="scope in consent.scopes" :key="scope" class="status-badge status-brand">
                                        {{ scopeNames[scope] || scope }}
                                    </span>
                                </div>
                            </div>
                        </div>
                        <div class="list-row-actions">
                            <UiButton
                                variant="outline"
                                class="!border-destructive/20 !text-destructive hover:!bg-destructive/5"
                                :disabled="pending"
                                :loading="pending"
                                @click="withdraw(consent.id, consent.name)"
                            >
                                撤回授权
                            </UiButton>
                        </div>
                    </article>
                </div>
                <p v-else class="empty-filter">没有匹配“{{ query }}”的授权记录。</p>
                <div v-if="data?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="state.next()">
                        加载更多
                    </UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
