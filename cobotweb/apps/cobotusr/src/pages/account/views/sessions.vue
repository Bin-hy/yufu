<script setup lang="ts">
defineOptions({ name: "AccountSessions" })
import { useRouter } from 'vue-router'
import { getCenters, getRps, revokeCenter, revokeRp, centerLogout } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useUserStore } from '@/pinia/stores/user'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const centers = useCursorPage('personal-centers', getCenters), rps = useCursorPage('personal-rps', getRps)
const { data: roots, status, error } = centers, { data: applications, status: rpStatus, error: rpError } = rps
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation, user = useUserStore(), router = useRouter()
const date = (seconds: number) => new Date(seconds * 1000).toLocaleString()
const live = (expires: number, revoked?: number | null) => !revoked && expires * 1000 > Date.now()
async function revoke(kind: 'center' | 'rp' | 'all', id = '') {
    const saved = await mutation.run(() => kind === 'all' ? centerLogout(true) : kind === 'rp' ? revokeRp(id) : revokeCenter(id))
    if (!saved) return
    if (kind === 'all' || (kind === 'center' && id === user.centerSession?.sso_session_id)) { user.logout(); await router.replace('/login'); return }
    await Promise.all([centers.refresh(), rps.refresh()]); emitToast({ type: 'success', message: '所选会话授权已撤销' })
}
</script>
<template>
    <div class="workspace-page"><PageHeader title="登录会话" description="中心会话关联多项应用登录，应用会话可单独撤销。"><UiButton variant="outline" :disabled="pending" @click="revoke('all')">退出全部中心与应用授权</UiButton></PageHeader>
        <p v-if="writeError" role="alert" class="inline-error mb-4">{{ writeError }}</p>
        <h2 class="mb-3 font-semibold">中心会话</h2><DataState :status="status" :error="error" :empty="!roots?.items.length" @retry="centers.refresh"><div class="space-y-3"><article v-for="root in roots?.items" :key="root.id" class="flex flex-wrap items-center justify-between gap-4 rounded-xl border bg-card p-5"><div><h3>{{ root.current ? '当前浏览器会话' : '其他中心会话' }} · {{ live(root.expires_at, root.revoked_at) ? '有效' : '已失效' }}</h3><p class="mt-2 text-xs text-muted-foreground">认证 {{ date(root.auth_time) }} · 到期 {{ date(root.expires_at) }}</p></div><UiButton v-if="live(root.expires_at, root.revoked_at)" variant="outline" :disabled="pending" @click="revoke('center', root.id)">退出此中心会话</UiButton></article></div></DataState><UiButton v-if="roots?.next" class="mt-3" variant="outline" @click="centers.next">下一页中心会话</UiButton>
        <h2 class="mb-3 mt-8 font-semibold">应用会话</h2><DataState :status="rpStatus" :error="rpError" :empty="!applications?.items.length" @retry="rps.refresh"><div class="space-y-3"><article v-for="rp in applications?.items" :key="rp.id" class="flex flex-wrap items-center justify-between gap-4 rounded-xl border bg-card p-5"><div><h3>{{ rp.name }} · {{ rp.environment === 'development' ? '开发' : '生产' }}</h3><p class="mt-2 text-xs text-muted-foreground">建立 {{ date(rp.created_at) }} · {{ live(rp.center_expires_at, rp.revoked_at) && !rp.center_revoked ? '有效' : '已失效' }}</p></div><UiButton v-if="live(rp.center_expires_at, rp.revoked_at) && !rp.center_revoked" variant="outline" :disabled="pending" @click="revoke('rp', rp.id)">撤销此应用会话</UiButton></article></div></DataState><UiButton v-if="applications?.next" class="mt-3" variant="outline" @click="rps.next">下一页应用会话</UiButton>
    </div>
</template>
