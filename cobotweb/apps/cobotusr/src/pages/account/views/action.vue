<script setup lang="ts">
defineOptions({ name: "AccountAction" })
import { computed, reactive, ref, onScopeDispose } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { consumeSecretFragment, exchangeAction, inspectAction, acceptInvitation, completePasswordReset } from '../api/actions'
import { useUserStore } from '@/pinia/stores/user'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { onSessionReset } from '@/workspace/session'
import AuthLayout from '@/layouts/auth-layout.vue'
import LoginForm from '@/components/workspace/login-form.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
const route = useRoute(), router = useRouter(), user = useUserStore()
let secret: { kind: string; secret: string } | null = null
let fragmentError = ''
try { secret = consumeSecretFragment(window.location, window.history) } catch (error) { fragmentError = (error as Error).message }
const actionId = ref(typeof route.query.action_id === 'string' ? route.query.action_id : '')
const form = reactive({ username: '', nickname: '', password: '', confirm: '' }), validation = ref(''), done = ref(false)
const state = useAsyncQuery({ key: () => actionId.value, loader: async signal => {
    if (fragmentError) throw new Error(fragmentError)
    if (secret) {
        const input = secret; secret = null
        try {
            const receipt = await exchangeAction({ ...input, flow_id: typeof route.query.flow_id === 'string' ? route.query.flow_id : undefined })
            actionId.value = receipt.action_id
            await router.replace({ name: 'account-action', query: { action_id: receipt.action_id } })
        } finally { input.secret = '' }
    }
    if (!actionId.value) throw new Error('需要有效的企业邀请或密码重置链接，请联系管理员。')
    return inspectAction(actionId.value, signal)
} })
const { data, status, error } = state, mutation = useAsyncMutation(), { pending, error: writeError } = mutation
const existing = computed(() => !!data.value?.requires_existing_account || !!user.centerSession)
const clear = () => { secret = null; form.password = ''; form.confirm = '' }
onScopeDispose(onSessionReset(clear)); onScopeDispose(clear)
async function submit() {
    validation.value = ''
    if (!data.value || done.value) return
    if ((data.value.kind === 'password_reset' || !existing.value) && (form.password.length < 8 || form.password !== form.confirm)) { validation.value = '新密码至少 8 位，两次输入应一致'; return }
    const saved = await mutation.run(() => data.value!.kind === 'password_reset'
        ? completePasswordReset(actionId.value, form.password)
        : acceptInvitation(existing.value ? { action_id: actionId.value, existing_account: true }
            : { action_id: actionId.value, existing_account: false, username: form.username.trim(), nickname: form.nickname.trim(), password: form.password }), async result => {
        done.value = true; clear()
        if (result.requires_login) user.logout()
        const flow = result.oidc_flow_id
        await router.replace(flow ? { name: 'oidc-consent', query: { flow_id: flow } } : result.requires_login ? { name: 'login' } : { name: 'account-profile' })
    })
    if (!saved) clear()
}
</script>
<template>
    <LoginForm v-if="data?.requires_existing_account && !user.centerSession" :action-id="actionId" />
    <AuthLayout v-else :title="data?.kind === 'password_reset' ? '重置密码' : '接受企业邀请'" description="企业统一身份认证">
        <DataState :status="status" :error="error" @retry="state.refresh">
            <form class="space-y-4" @submit.prevent="submit">
                <p class="text-sm">{{ data?.kind === 'password_reset' ? '设置新密码后，原有登录会话会失效。' : existing ? '以当前账户接受邀请，保留同一个统一身份。' : '此邀请允许创建一个企业账户。' }}</p>
                <p v-if="data?.kind === 'invitation'" class="text-xs text-muted-foreground">{{ data.person_kind === 'employee' ? '员工账户' : '合作方账户' }}{{ data.developer_grant ? ' · 授予开发者资格' : '' }}</p>
                <template v-if="data?.kind === 'invitation' && !existing">
                    <div class="form-field"><label for="invitation-username">用户名</label><UiInput id="invitation-username" v-model="form.username" autocomplete="username" required :disabled="pending" /></div>
                    <div class="form-field"><label for="invitation-nickname">显示名称</label><UiInput id="invitation-nickname" v-model="form.nickname" required :disabled="pending" /></div>
                    <p class="text-xs">用户名已存在时，请先登录该账户，再重新打开邀请链接。</p>
                </template>
                <template v-if="data?.kind === 'password_reset' || !existing">
                    <div class="form-field"><label for="action-password">新密码</label><UiInput id="action-password" v-model="form.password" type="password" autocomplete="new-password" required :disabled="pending" /></div>
                    <div class="form-field"><label for="action-confirm">确认新密码</label><UiInput id="action-confirm" v-model="form.confirm" type="password" autocomplete="new-password" required :disabled="pending" /></div>
                </template>
                <p v-if="validation || writeError" class="inline-error" role="alert">{{ validation || writeError }}</p>
                <UiButton class="w-full" type="submit" :disabled="pending || done">{{ pending ? '正在处理…' : data?.kind === 'password_reset' ? '完成重置' : '接受邀请' }}</UiButton>
            </form>
        </DataState>
    </AuthLayout>
</template>
