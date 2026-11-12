<script setup lang="ts">
defineOptions({ name: 'AccountAction' })
import { computed, onScopeDispose, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Check, KeyRound, MailCheck, ShieldCheck } from '@lucide/vue'
import {
    consumeSecretFragment,
    exchangeAction,
    inspectAction,
    acceptInvitation,
    completePasswordReset,
} from '../api/actions'
import { useUserStore } from '@/pinia/stores/user'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { onSessionReset } from '@/workspace/session'
import AuthLayout from '@/layouts/auth-layout.vue'
import LoginForm from '@/components/workspace/login-form.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
const route = useRoute(),
    router = useRouter(),
    user = useUserStore()
let secret: { kind: string; secret: string } | null = null
let fragmentError = ''
try {
    secret = consumeSecretFragment(window.location, window.history)
} catch (error) {
    fragmentError = (error as Error).message
}
const actionId = ref(typeof route.query.action_id === 'string' ? route.query.action_id : '')
const form = reactive({ username: '', nickname: '', password: '', confirm: '' }),
    validation = ref(''),
    done = ref(false)
const state = useAsyncQuery({
    key: () => actionId.value,
    loader: async (signal) => {
        if (fragmentError) throw new Error(fragmentError)
        if (secret) {
            const input = secret
            secret = null
            try {
                const receipt = await exchangeAction({
                    ...input,
                    flow_id: typeof route.query.flow_id === 'string' ? route.query.flow_id : undefined,
                })
                actionId.value = receipt.action_id
                await router.replace({ name: 'account-action', query: { action_id: receipt.action_id } })
            } finally {
                input.secret = ''
            }
        }
        if (!actionId.value) throw new Error('需要有效的企业邀请或密码重置链接，请联系管理员。')
        return inspectAction(actionId.value, signal)
    },
})
const { data, status, error } = state,
    mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation
const existing = computed(() => !!data.value?.requires_existing_account || !!user.centerSession)
const isReset = computed(() => data.value?.kind === 'password_reset')
const passwordMatch = computed(() => !!form.confirm && form.password === form.confirm)
const clear = () => {
    secret = null
    form.password = ''
    form.confirm = ''
}
onScopeDispose(onSessionReset(clear))
onScopeDispose(clear)
async function submit() {
    validation.value = ''
    if (!data.value || done.value) return
    if (
        (isReset.value || !existing.value) &&
        (form.password.length < 8 || form.password !== form.confirm)
    ) {
        validation.value = '新密码至少 8 位，两次输入应一致'
        document
            .getElementById(form.password.length < 8 ? 'action-password' : 'action-confirm')
            ?.focus()
        return
    }
    const saved = await mutation.run(
        () =>
            isReset.value
                ? completePasswordReset(actionId.value, form.password)
                : acceptInvitation(
                      existing.value
                          ? { action_id: actionId.value, existing_account: true }
                          : {
                                action_id: actionId.value,
                                existing_account: false,
                                username: form.username.trim(),
                                nickname: form.nickname.trim(),
                                password: form.password,
                            },
                  ),
        async (result) => {
            done.value = true
            clear()
            if (result.requires_login) user.logout()
            const flow = result.oidc_flow_id
            await router.replace(
                flow
                    ? { name: 'oidc-consent', query: { flow_id: flow } }
                    : result.requires_login
                      ? { name: 'login' }
                      : { name: 'account-profile' },
            )
        },
    )
    if (!saved) clear()
}
</script>
<template>
    <LoginForm v-if="data?.requires_existing_account && !user.centerSession" :action-id="actionId" />
    <AuthLayout
        v-else
        :title="isReset ? '重置密码' : '接受企业邀请'"
        description="企业统一身份认证"
    >
        <DataState :status="status" :error="error" :skeleton-rows="2" @retry="state.refresh()">
            <form class="space-y-4" novalidate @submit.prevent="submit">
                <div class="inline-notice">
                    <ShieldCheck :size="16" class="mt-0.5 shrink-0 text-primary" aria-hidden="true" />
                    <p>
                        <template v-if="isReset">设置新密码后，原有登录会话会全部失效。</template>
                        <template v-else-if="existing">以当前账户接受邀请，保留同一个统一身份。</template>
                        <template v-else>此邀请允许创建一个企业账户，请确认信息后继续。</template>
                    </p>
                </div>
                <div v-if="data?.kind === 'invitation'" class="flex flex-wrap gap-2">
                    <span class="status-badge status-brand">
                        <MailCheck :size="11" aria-hidden="true" />
                        {{ data.person_kind === 'employee' ? '员工账户' : '合作方账户' }}
                    </span>
                    <span v-if="data.developer_grant" class="status-badge status-info">开发者资格</span>
                </div>
                <template v-if="data?.kind === 'invitation' && !existing">
                    <UiFormField id="invitation-username" label="用户名" required>
                        <UiInput
                            id="invitation-username"
                            v-model="form.username"
                            autocomplete="username"
                            required
                            :disabled="pending"
                            placeholder="请输入登录用户名"
                        />
                    </UiFormField>
                    <UiFormField id="invitation-nickname" label="显示名称" required>
                        <UiInput
                            id="invitation-nickname"
                            v-model="form.nickname"
                            required
                            :disabled="pending"
                            placeholder="请输入显示名称"
                        />
                    </UiFormField>
                    <p class="form-help">用户名已存在时，请先登录该账户，再重新打开邀请链接。</p>
                </template>
                <template v-if="isReset || !existing">
                    <UiFormField id="action-password" label="新密码" required hint="至少 8 位字符，建议混合大小写与数字。">
                        <UiInput
                            id="action-password"
                            v-model="form.password"
                            type="password"
                            autocomplete="new-password"
                            required
                            :disabled="pending"
                            placeholder="设置新密码"
                        />
                    </UiFormField>
                    <UiFormField id="action-confirm" label="确认新密码" required>
                        <UiInput
                            id="action-confirm"
                            v-model="form.confirm"
                            type="password"
                            autocomplete="new-password"
                            required
                            :disabled="pending"
                            placeholder="再次输入新密码"
                        />
                        <p
                            v-if="form.confirm"
                            class="form-help flex items-center gap-1.5"
                            :class="passwordMatch ? 'text-success' : 'text-destructive'"
                        >
                            <Check :size="12" aria-hidden="true" />
                            {{ passwordMatch ? '两次输入一致' : '两次输入不一致' }}
                        </p>
                    </UiFormField>
                </template>
                <p v-if="validation || writeError" class="form-alert" role="alert">
                    {{ validation || writeError }}
                </p>
                <UiButton type="submit" size="lg" block :disabled="pending || done" :loading="pending">
                    <KeyRound v-if="!pending" :size="15" aria-hidden="true" />
                    {{ isReset ? '完成重置' : '接受邀请' }}
                </UiButton>
            </form>
        </DataState>
    </AuthLayout>
</template>
