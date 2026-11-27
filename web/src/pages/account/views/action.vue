<script setup lang="ts">
defineOptions({ name: 'AccountAction' })
import { computed, onScopeDispose, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Check, KeyRound, MailCheck, ShieldCheck } from '@lucide/vue'
import {
    consumeSecretFragment,
    exchangeAction,
    inspectAction,
    acceptInvitation,
    completePasswordReset,
} from '../api/actions'
import { getExternalProviders, startExternalBind, type ExternalProvider } from '../api/identities'
import ExternalProviderIcon from '@/components/brand/external-provider-icon.vue'
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
const manualMode = ref(!actionId.value && !secret)
const manualCode = ref('')
const manualError = ref(fragmentError)
const exchanging = ref(false)
const form = reactive({ username: '', nickname: '', password: '', confirm: '' }),
    validation = ref(''),
    bindError = ref(''),
    providers = ref<ExternalProvider[]>([]),
    selectedProvider = ref(''),
    providerController = new AbortController(),
    done = ref(false)
const bindableProviders = computed(() =>
    providers.value.filter((provider) => provider.mode === 'oauth'),
)
async function loadProviders() {
    try {
        providers.value = await getExternalProviders(providerController.signal)
    } catch {
        providers.value = []
    }
}
const state = useAsyncQuery({
    key: () => actionId.value,
    active: () => !manualMode.value,
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
            } catch (error) {
                manualMode.value = true
                manualError.value = error instanceof Error ? error.message : '邀请链接无效，请重新输入邀请码'
                throw error
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
onScopeDispose(() => providerController.abort())
watch(
    data,
    (value) => {
        if (value?.kind === 'invitation' && !existing.value && !providers.value.length) {
            void loadProviders()
        }
    },
    { immediate: true },
)
watch(existing, (value) => {
    if (value) selectedProvider.value = ''
})

function parseManualAction(raw: string) {
    const value = raw.trim()
    let secret = value
    let kind = 'invitation'
    try {
        const url = new URL(value)
        const hash = new URLSearchParams(url.hash.replace(/^#/, ''))
        const fromHash = hash.get('secret')
        const fromQuery = url.searchParams.get('secret')
        const parsed = fromHash ?? fromQuery ?? ''
        if (parsed) {
            secret = parsed
            const parsedKind = hash.get('kind') ?? url.searchParams.get('kind')
            if (parsedKind === 'password_reset') kind = 'password_reset'
        }
    } catch {
        // A raw 43-character secret is not a URL; use it as-is.
    }
    if (!/^[A-Za-z0-9_-]{43}$/.test(secret)) {
        throw new Error('邀请码格式不正确，请粘贴管理员提供的 43 位邀请码或完整邀请链接')
    }
    return { kind, secret }
}

async function exchangeManual() {
    if (exchanging.value) return
    manualError.value = ''
    try {
        const parsed = parseManualAction(manualCode.value)
        exchanging.value = true
        const receipt = await exchangeAction({
            ...parsed,
            flow_id: typeof route.query.flow_id === 'string' ? route.query.flow_id : undefined,
        })
        manualCode.value = ''
        manualMode.value = false
        actionId.value = receipt.action_id
        await router.replace({ name: 'account-action', query: { action_id: receipt.action_id } })
    } catch (error) {
        manualError.value = error instanceof Error ? error.message : '邀请码无效，请联系管理员重新生成'
    } finally {
        exchanging.value = false
    }
}

async function submit() {
    validation.value = ''
    bindError.value = ''
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
                                bind_provider: selectedProvider.value || undefined,
                            },
                  ),
        async (result) => {
            done.value = true
            clear()
            if (result.requires_login) user.logout()
            if (!isReset.value) await user.ensureSession()
            const flow = result.oidc_flow_id
            if (result.bind_provider) {
                try {
                    const bind = await startExternalBind(result.bind_provider, flow ?? undefined)
                    window.location.assign(bind.authorize_url)
                } catch (error) {
                    bindError.value =
                        '账户已创建，但第三方绑定未开始：' +
                        (error instanceof Error ? error.message : '请稍后重试')
                    await router.replace(
                        flow
                            ? { name: 'oidc-consent', query: { flow_id: flow } }
                            : {
                                  name: 'account-identities',
                                  query: { error: 'external_start_failed', bind: result.bind_provider },
                              },
                    )
                }
                return
            }
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
    <AuthLayout
        v-if="manualMode"
        title="输入邀请码"
        description="输入邀请码或账户操作链接"
    >
        <form class="space-y-4" novalidate @submit.prevent="exchangeManual">
            <p v-if="manualError" class="form-alert" role="alert">{{ manualError }}</p>
            <UiFormField
                id="invitation-code"
                label="邀请码或邀请链接"
                required
                hint="一次性凭证。"
            >
                <UiInput
                    id="invitation-code"
                    v-model="manualCode"
                    :disabled="exchanging"
                    required
                    autofocus
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="粘贴 43 位邀请码或完整邀请链接"
                />
            </UiFormField>
            <UiButton type="submit" size="lg" block :loading="exchanging" :disabled="exchanging">
                <KeyRound v-if="!exchanging" :size="15" aria-hidden="true" />
                继续
            </UiButton>
            <div class="auth-links">
                <span>已有企业账户？</span>
                <RouterLink :to="{ name: 'login' }">返回登录</RouterLink>
            </div>
        </form>
    </AuthLayout>
    <LoginForm v-else-if="data?.requires_existing_account && !user.centerSession" :action-id="actionId" />
    <AuthLayout
        v-else
        :title="isReset ? '重置密码' : '接受企业邀请'"
        description="确认账户操作"
    >
        <DataState :status="status" :error="error" :skeleton-rows="2" @retry="state.refresh()">
            <form class="space-y-4" novalidate @submit.prevent="submit">
                <div class="inline-notice">
                    <ShieldCheck :size="16" class="mt-0.5 shrink-0 text-primary" aria-hidden="true" />
                    <p>
                        <template v-if="isReset">设置后原会话全部失效。</template>
                        <template v-else-if="existing">以当前账户接受邀请。</template>
                        <template v-else>创建企业账户。</template>
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
                    <p class="form-help">用户名已存在时，请先登录再打开邀请。</p>
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
                <div
                    v-if="data?.kind === 'invitation' && !existing && bindableProviders.length"
                    class="space-y-2"
                >
                    <p class="text-[11px] font-semibold text-muted-foreground">同步绑定第三方身份（可选）</p>
                    <div class="grid grid-cols-2 gap-2" role="group" aria-label="选择要同步绑定的第三方身份">
                        <button
                            v-for="provider in bindableProviders"
                            :key="provider.key"
                            type="button"
                            class="flex items-center gap-2 rounded-lg border p-2.5 text-left transition-colors disabled:opacity-60"
                            :class="
                                selectedProvider === provider.key
                                    ? 'border-primary bg-primary/5'
                                    : 'border-border bg-card hover:bg-accent/30'
                            "
                            :aria-pressed="selectedProvider === provider.key"
                            :disabled="pending"
                            @click="selectedProvider = selectedProvider === provider.key ? '' : provider.key"
                        >
                            <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border bg-background">
                                <ExternalProviderIcon :provider="provider.key" :size="17" />
                            </span>
                            <span class="min-w-0 flex-1 text-xs font-semibold">{{ provider.label }}</span>
                            <Check
                                v-if="selectedProvider === provider.key"
                                :size="14"
                                class="shrink-0 text-primary"
                                aria-hidden="true"
                            />
                        </button>
                    </div>
                    <p class="form-help">不选择则仅创建账户，之后仍可在账户中心绑定。</p>
                </div>
                <p v-if="validation || writeError || bindError" class="form-alert" role="alert">
                    {{ validation || writeError || bindError }}
                </p>
                <UiButton type="submit" size="lg" block :disabled="pending || done" :loading="pending">
                    <KeyRound v-if="!pending" :size="15" aria-hidden="true" />
                    {{ isReset ? '完成重置' : '接受邀请' }}
                </UiButton>
            </form>
        </DataState>
    </AuthLayout>
</template>
