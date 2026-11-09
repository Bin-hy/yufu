<script setup lang="ts">
defineOptions({ name: "AdminAccounts" })
import { reactive, ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { listAccounts, setAccount, issueInvitation, issueReset, revokeAction, setCandidate } from '../api'
import type { EnterpriseAccount, IssuedAction } from '../api/type'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult } from '@/composables/use-ephemeral-result'
import { useUserStore } from '@/pinia/stores/user'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
const list = useCursorPage('enterprise-accounts', listAccounts), { data, status, error } = list
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation, user = useUserStore(), router = useRouter()
const opened = ref(false), form = reactive({ purpose: 'onboarding' as 'onboarding' | 'qualification', target: 0, person_kind: 'employee' as 'employee' | 'collaborator', developer_grant: false })
const candidate = reactive({ requester: 0, target: 0, allowed: true })
const issued = useEphemeralResult<{ action: IssuedAction; reset: boolean }>(), result = issued.value
const link = computed(() => {
    if (!result.value) return ''
    return new URL(router.resolve({ name: 'account-action' }).href, window.location.origin).href + '#kind=' + (result.value.reset ? 'password_reset' : 'invitation') + '&secret=' + encodeURIComponent(result.value.action.secret)
})
async function invite() { await mutation.run(() => issueInvitation({ purpose: form.purpose, target_user_id: form.target || null, person_kind: form.person_kind, developer_grant: form.developer_grant }), value => { result.value = { action: value, reset: false }; opened.value = false }) }
async function reset(account: EnterpriseAccount) { await mutation.run(() => issueReset(account.user_id), value => { result.value = { action: value, reset: true } }) }
async function change(account: EnterpriseAccount, field: 'enabled' | 'developer') {
    await mutation.run(() => setAccount(account.user_id, field, !(field === 'developer' ? account.developer_enabled : account.enabled), account.revision), async () => {
        await list.refresh()
        if (account.user_id === user.centerSession?.user_id) { await user.ensureSession(); if (!user.centerSession) await router.replace('/login') }
    })
}
async function revoke() { if (result.value) await mutation.run(() => revokeAction(result.value!.action.id, result.value!.reset), issued.clear) }
async function allowCandidate() { await mutation.run(() => setCandidate(candidate.requester, candidate.target, candidate.allowed)) }
</script>
<template>
    <div class="workspace-page"><PageHeader title="企业账户与邀请" description="受控邀请创建或扩充账户资格；开发者资格不自动授予生产应用访问。"><UiButton @click="opened = !opened">签发邀请</UiButton></PageHeader>
        <p v-if="writeError" class="inline-error mb-4" role="alert">{{ writeError }}</p>
        <section v-if="result" class="mb-5 rounded-xl border border-primary bg-card p-5"><h2 class="font-semibold">{{ result.reset ? '密码重置链接' : '企业邀请链接' }}（仅此次展示）</h2><p class="mt-2 text-xs">到期 {{ new Date(result.action.expires_at * 1000).toLocaleString() }}</p><code class="my-3 block select-all break-all rounded bg-muted p-3 text-xs">{{ link }}</code><p class="text-xs text-muted-foreground">通过企业认可的渠道交给指定接收人。离开页面后清除链接。</p><div class="mt-3 flex gap-3"><UiButton variant="outline" :disabled="pending" @click="revoke">提前撤销此链接</UiButton><UiButton variant="outline" @click="issued.clear">隐藏链接</UiButton></div></section>
        <form v-if="opened" class="mb-5 space-y-4 rounded-xl border bg-card p-5" @submit.prevent="invite"><div class="grid gap-4 md:grid-cols-2"><div class="form-field"><label for="invitation-purpose">邀请用途</label><select id="invitation-purpose" v-model="form.purpose" class="ui-select"><option value="onboarding">开户邀请（可接受到已有账户）</option><option value="qualification">已有账户资格邀请</option></select></div><div class="form-field"><label for="invitation-target">限定接收账户 ID（开户可留空）</label><UiInput id="invitation-target" v-model.number="form.target" type="number" min="0" :required="form.purpose === 'qualification'" /></div><div class="form-field"><label for="invitation-person">人员类型</label><select id="invitation-person" v-model="form.person_kind" class="ui-select"><option value="employee">企业员工</option><option value="collaborator">受邀合作方</option></select></div><label class="flex items-center gap-2 text-sm"><input v-model="form.developer_grant" type="checkbox">明确授予开发者资格</label></div><UiButton type="submit" :disabled="pending">签发一次性邀请</UiButton></form>
        <DataState :status="status" :error="error" :empty="!data?.items.length" @retry="list.refresh"><div class="space-y-3"><article v-for="account in data?.items" :key="account.user_id" class="flex flex-wrap items-center justify-between gap-4 rounded-xl border bg-card p-5"><div><h2 class="font-semibold">{{ account.nickname || account.username }} · {{ account.user_id }}</h2><p class="mt-2 text-xs text-muted-foreground">{{ account.username }} · {{ account.person_kind === 'employee' ? '员工' : '合作方' }} · {{ account.enabled ? '启用' : '停用' }} · {{ account.developer_enabled ? '具有开发者资格' : '普通账户' }}</p></div><div class="flex flex-wrap gap-2"><UiButton variant="outline" :disabled="pending" @click="change(account, 'enabled')">{{ account.enabled ? '停用并撤销登录' : '启用账户' }}</UiButton><UiButton variant="outline" :disabled="pending" @click="change(account, 'developer')">{{ account.developer_enabled ? '撤销开发者资格' : '授予开发者资格' }}</UiButton><UiButton variant="outline" :disabled="pending" @click="reset(account)">签发密码重置</UiButton></div></article></div></DataState><UiButton v-if="data?.next" class="mt-4" variant="outline" @click="list.next">下一页</UiButton>
        <section class="mt-6 rounded-xl border bg-card p-5"><h2 class="font-semibold">合作方候选可见性</h2><p class="my-3 text-xs text-muted-foreground">只为明确合作关系授予候选可见性，不开放企业人员目录。</p><form class="flex flex-wrap items-end gap-3" @submit.prevent="allowCandidate"><div class="form-field"><label for="candidate-requester">请求方账户 ID</label><UiInput id="candidate-requester" v-model.number="candidate.requester" type="number" min="1" required /></div><div class="form-field"><label for="candidate-target">候选账户 ID</label><UiInput id="candidate-target" v-model.number="candidate.target" type="number" min="1" required /></div><label class="mb-2 flex gap-2 text-sm"><input v-model="candidate.allowed" type="checkbox">允许可见</label><UiButton type="submit" :disabled="pending">更新候选范围</UiButton></form></section>
    </div>
</template>
