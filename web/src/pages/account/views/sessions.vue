<script setup lang="ts">
defineOptions({ name: 'AccountSessions' })
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { Clock, Laptop, LogOut, ShieldAlert, ShieldCheck } from '@lucide/vue'
import { getCenters, getRps, revokeCenter, revokeRp, centerLogout } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useUserStore } from '@/pinia/stores/user'
import { emitToast } from '@/utils/toast'
import FailureFeedback from '@/components/Feedback/failure-feedback.vue'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const centers = useCursorPage('personal-centers', getCenters),
    rps = useCursorPage('personal-rps', getRps)
const { data: roots, status, error, refreshing } = centers,
    { data: applications, status: rpStatus, error: rpError, refreshing: rpRefreshing } = rps
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation,
    user = useUserStore(),
    router = useRouter()
const refreshSessions = () => Promise.all([centers.refresh(), rps.refresh()])
const date = (seconds: number) => new Date(seconds * 1000).toLocaleString('zh-CN')
const live = (expires: number, revoked?: number | null) => !revoked && expires * 1000 > Date.now()
const activeCenters = computed(
    () => roots.value?.items.filter((root) => live(root.expires_at, root.revoked_at)).length ?? 0,
)
const activeRps = computed(
    () =>
        applications.value?.items.filter(
            (rp) => live(rp.center_expires_at, rp.revoked_at) && !rp.center_revoked,
        ).length ?? 0,
)
async function revoke(kind: 'center' | 'rp' | 'all', id = '') {
    const write = () => kind === 'all' ? centerLogout(true) : kind === 'rp' ? revokeRp(id) : revokeCenter(id)
    const saved = kind !== 'rp' ? await mutation.confirm({
        title: kind === 'all' ? '退出全部会话' : '退出此中心会话',
        content: kind === 'all'
            ? '所有中心会话与应用授权都会被撤销，当前浏览器也会退出登录。'
            : '该中心会话及其关联的应用授权将立即失效。',
        confirmText: kind === 'all' ? '退出全部会话' : '退出此会话',
        destructive: true,
    }, write) : await mutation.run(write)
    if (!saved) return
    if (kind === 'all' || (kind === 'center' && id === user.centerSession?.sso_session_id)) {
        user.logout()
        await router.replace('/login')
        return
    }
    await Promise.all([centers.refresh(), rps.refresh()])
    emitToast({ type: 'success', message: '所选会话授权已撤销' })
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="登录会话"
            eyebrow="SESSIONS"
            description="中心会话关联多项应用登录，应用会话可以单独撤销。发现异常访问时，先退出全部会话。"
        >
            <UiButton variant="destructive" :disabled="pending" :loading="pending" @click="revoke('all')">
                <LogOut v-if="!pending" :size="14" aria-hidden="true" />退出全部中心与应用授权
            </UiButton>
        </PageHeader>
        <FailureFeedback :failure="mutation.failure.value" :message="writeError"
            :recovery-label="['reload', 'check-result'].includes(mutation.failure.value?.recovery ?? '') ? '重新加载并核对' : undefined"
            :recover="refreshSessions" />

        <div class="metric-grid mb-5">
            <article class="metric-card">
                <span class="metric-label"><ShieldCheck :size="13" aria-hidden="true" />有效中心会话</span>
                <strong class="metric-value">{{ activeCenters }}</strong>
                <span class="metric-meta">当前账户的中心登录会话</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><Laptop :size="13" aria-hidden="true" />有效应用会话</span>
                <strong class="metric-value">{{ activeRps }}</strong>
                <span class="metric-meta">仍可访问的应用登录</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><Clock :size="13" aria-hidden="true" />当前会话到期</span>
                <strong class="metric-value !text-sm">
                    {{ user.centerSession?.expires_at ? date(user.centerSession.expires_at) : '—' }}
                </strong>
                <span class="metric-meta">到期后需要重新认证</span>
            </article>
        </div>

        <section class="panel mb-5">
            <header class="panel-heading">
                <div><h2>中心会话</h2><p>同一浏览器中的中心登录记录。</p></div>
                <span class="status-badge status-muted">{{ roots?.items.length ?? 0 }} 条</span>
            </header>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!roots?.items.length"
                empty-title="没有中心会话"
                empty-description="登录后会在这里显示当前浏览器的中心会话。"
                :skeleton-rows="2"
                @retry="centers.refresh()"
            >
                <article v-for="root in roots?.items" :key="root.id" class="list-row">
                    <div class="list-row-main">
                        <span class="list-icon">
                            <ShieldCheck :size="16" aria-hidden="true" />
                        </span>
                        <div>
                            <div class="flex flex-wrap items-center gap-2">
                                <h3 class="list-title">{{ root.current ? '当前浏览器会话' : '其他中心会话' }}</h3>
                                <span class="status-badge" :class="live(root.expires_at, root.revoked_at) ? 'status-success' : 'status-muted'">
                                    {{ live(root.expires_at, root.revoked_at) ? '有效' : '已失效' }}
                                </span>
                                <span v-if="root.current" class="status-badge status-brand">本机</span>
                            </div>
                            <p class="list-meta">认证 {{ date(root.auth_time) }} · 到期 {{ date(root.expires_at) }}</p>
                        </div>
                    </div>
                    <div class="list-row-actions">
                        <UiButton
                            v-if="live(root.expires_at, root.revoked_at)"
                            variant="outline"
                            class="!border-destructive/20 !text-destructive hover:!bg-destructive/5"
                            :disabled="pending"
                            @click="revoke('center', root.id)"
                        >
                            退出此中心会话
                        </UiButton>
                    </div>
                </article>
                <div v-if="roots?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="centers.next()">下一页中心会话</UiButton>
                </div>
            </DataState>
        </section>

        <section class="panel">
            <header class="panel-heading">
                <div><h2>应用会话</h2><p>应用登录可以独立撤销，不影响其他应用。</p></div>
                <span class="status-badge status-muted">{{ applications?.items.length ?? 0 }} 条</span>
            </header>
            <DataState
                :status="rpStatus"
                :error="rpError"
                :refreshing="rpRefreshing"
                :empty="!applications?.items.length"
                empty-title="没有应用会话"
                empty-description="从应用入口登录后，应用会话会显示在这里。"
                :skeleton-rows="2"
                @retry="rps.refresh()"
            >
                <article v-for="rp in applications?.items" :key="rp.id" class="list-row">
                    <div class="list-row-main">
                        <span class="list-icon"><Laptop :size="16" aria-hidden="true" /></span>
                        <div>
                            <div class="flex flex-wrap items-center gap-2">
                                <h3 class="list-title">{{ rp.name }}</h3>
                                <span class="status-badge" :class="rp.environment === 'production' ? 'status-success' : 'status-info'">
                                    {{ rp.environment === 'development' ? '开发' : '生产' }}
                                </span>
                                <span
                                    class="status-badge"
                                    :class="live(rp.center_expires_at, rp.revoked_at) && !rp.center_revoked ? 'status-success' : 'status-muted'"
                                >
                                    {{ live(rp.center_expires_at, rp.revoked_at) && !rp.center_revoked ? '有效' : '已失效' }}
                                </span>
                            </div>
                            <p class="list-meta">建立 {{ date(rp.created_at) }} · 中心会话到期 {{ date(rp.center_expires_at) }}</p>
                        </div>
                    </div>
                    <div class="list-row-actions">
                        <UiButton
                            v-if="live(rp.center_expires_at, rp.revoked_at) && !rp.center_revoked"
                            variant="outline"
                            :disabled="pending"
                            @click="revoke('rp', rp.id)"
                        >撤销此应用会话</UiButton>
                        <span v-else class="inline-flex items-center gap-1.5 text-[10.5px] text-muted-foreground">
                            <ShieldAlert :size="12" aria-hidden="true" />已失效
                        </span>
                    </div>
                </article>
                <div v-if="applications?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="rpRefreshing" :disabled="rpRefreshing" @click="rps.next()">下一页应用会话</UiButton>
                </div>
            </DataState>
        </section>

        <div class="mt-5 inline-notice is-warning">
            <ShieldAlert :size="15" class="mt-0.5 shrink-0 text-warning" aria-hidden="true" />
            <p><strong>安全提示：</strong>如果你不认识某个应用会话，请立即撤销，并修改账户密码以撤销全部授权。</p>
        </div>
    </div>
</template>
