<script setup lang="ts">
import { computed, reactive, ref, onMounted, onScopeDispose, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { ArrowRight, Info } from '@lucide/vue'
import { centerLogin } from '@/pages/account/api'
import { getExternalProviders, startExternalLogin, type ExternalProvider } from '@/pages/account/api/identities'
import ExternalProviderIcon from '@/components/brand/external-provider-icon.vue'
import InfoHint from '@/components/workspace/info-hint.vue'
import { useUserStore } from '@/pinia/stores/user'
import { useTabsStore } from '@/pinia/stores/tabs'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { setRememberUser, removeRememberUser, getRememberUser } from '@/common/cache/localstorage'
import { resetSession, sessionOwner, sessionExpiredNotice } from '@/workspace/session'
import { loadWorkspace } from '@/storage/workspace'
import { canEnter } from '@/router/guard'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import AuthLayout from '@/layouts/auth-layout.vue'
const props = defineProps<{ oidc?: boolean; flowId?: string; actionId?: string }>(),
    router = useRouter(),
    route = useRoute(),
    user = useUserStore(),
    tabs = useTabsStore()
const remembered = getRememberUser(),
    form = reactive({
        username: props.oidc ? '' : (remembered ?? ''),
        password: '',
        remember: !props.oidc && !!remembered,
    }),
    validation = ref(''),
    capsLock = ref(false),
    mutation = useAsyncMutation(),
    { pending, error } = mutation
const providers = ref<ExternalProvider[]>([]),
    externalPending = ref(''),
    externalError = ref(''),
    providerController = new AbortController()
onMounted(async () => {
    try {
        providers.value = await getExternalProviders(providerController.signal)
    } catch {
        providers.value = []
    }
})
onScopeDispose(() => providerController.abort())
async function externalLogin(provider: ExternalProvider) {
    if (pending.value || externalPending.value) return
    externalError.value = ''
    if (provider.mode === 'native') {
        externalError.value = '请打开 Yufu 微信小程序完成登录'
        return
    }
    externalPending.value = provider.key
    try {
        const flowId = props.flowId || (typeof route.query.flow_id === 'string' ? route.query.flow_id : undefined)
        const result = await startExternalLogin(provider.key, flowId)
        window.location.assign(result.authorize_url)
    } catch (e) {
        externalError.value = e instanceof Error ? e.message : '无法发起第三方登录'
    } finally {
        externalPending.value = ''
    }
}
function providerHint(provider: ExternalProvider) {
    if (provider.mode === 'native') return '请打开 Yufu 微信小程序'
    if (provider.key === 'github') return '使用 GitHub 账号继续'
    if (provider.key === 'qq') return '使用 QQ 账号继续'
    if (provider.key === 'wechat_qr') return '使用微信扫码继续'
    return '使用第三方账号继续'
}
const retrying = ref(false)
const restoreUnavailable = computed(() => !props.oidc && !props.actionId && (!!user.identityError || retrying.value))
function workspaceTarget() {
    const path = typeof route.query.redirect === 'string' ? route.query.redirect : '/account'
    return path.startsWith('/') && !path.startsWith('//') && router.resolve(path).meta.workspace ? path : '/account'
}
async function retrySession() {
    if (retrying.value) return
    retrying.value = true
    try {
        await user.ensureSession()
        if (user.centerSession && !user.identityError) await router.replace(workspaceTarget())
    } finally { retrying.value = false }
}
watch(
    form,
    () => {
        validation.value = ''
    },
    { deep: true },
)
onScopeDispose(() => { form.password = '' })
function detectCaps(event: KeyboardEvent) {
    capsLock.value = event.getModifierState?.('CapsLock') ?? false
}
async function submit() {
    if (pending.value || restoreUnavailable.value) return
    validation.value = ''
    if (!form.username.trim() || !form.password) {
        validation.value = '请输入用户名和密码'
        document.getElementById(!form.username.trim() ? 'login-username' : 'login-password')?.focus()
        return
    }
    resetSession()
    const saved = await mutation.run(
        () =>
            centerLogin({
                username: form.username.trim(),
                password: form.password,
                flow_id:
                    props.flowId ||
                    (typeof route.query.flow_id === 'string' ? route.query.flow_id : undefined),
            }),
        async (result) => {
            user.setCenterSession(result)
            form.password = ''
            if (!props.oidc) {
                if (form.remember) setRememberUser(form.username.trim())
                else removeRememberUser()
            }
            await user.ensureSession()
            const snapshot = loadWorkspace(router, sessionOwner.value)
            if (snapshot)
                for (const tab of snapshot.tabs) {
                    const restored = router.resolve(tab.fullPath)
                    if (canEnter(restored.meta, user.centerSession)) tabs.sync(restored, false)
                }
        },
    )
    form.password = ''
    if (saved) {
        if (props.actionId) {
            await router.replace({ name: 'account-action', query: { action_id: props.actionId } })
        } else if (props.oidc) {
            const flow =
                props.flowId ||
                (typeof route.query.flow_id === 'string' ? route.query.flow_id : '')
            window.location.assign(
                router.resolve({ name: 'oidc-consent', query: { flow_id: flow } }).href,
            )
        } else {
            await router.replace(workspaceTarget())
        }
    }
}
</script>
<template>
    <AuthLayout
        :oidc="oidc"
        :title="restoreUnavailable ? '会话状态不可用' : oidc ? '登录并授权' : '登录'"
        :description="
            restoreUnavailable
                ? '恢复连接后可重试。'
                : oidc
                ? '使用企业账户继续授权。'
                : '企业统一身份认证'
        "
    >
        <div v-if="restoreUnavailable" class="space-y-4" role="status">
            <p class="form-alert">{{ user.identityError || '正在重新验证登录状态…' }}</p>
            <UiButton block :loading="retrying" :disabled="retrying" @click="retrySession">重新验证登录状态</UiButton>
        </div>
        <form v-else class="space-y-4" novalidate @submit.prevent="submit">
            <p v-if="sessionExpiredNotice" class="form-alert" role="status">中心会话已失效，请重新登录后继续。</p>
            <p v-if="validation || error || externalError" class="form-alert" role="alert">
                {{ validation || error || externalError }}
            </p>
            <UiFormField id="login-username" label="用户名" required>
                <UiInput
                    id="login-username"
                    v-model="form.username"
                    autocomplete="username"
                    :disabled="pending"
                    required
                    autofocus
                    :invalid="!!validation && !form.username.trim()"
                    placeholder="请输入企业账户用户名"
                />
            </UiFormField>
            <UiFormField id="login-password" label="密码" required>
                <template #aside>
                    <InfoHint text="忘记密码请联系管理员重置" label="忘记密码说明" />
                </template>
                <UiInput
                    id="login-password"
                    v-model="form.password"
                    type="password"
                    autocomplete="current-password"
                    :disabled="pending"
                    required
                    :invalid="!!validation && !form.password"
                    placeholder="请输入账户密码"
                    @keydown="detectCaps"
                    @keyup="detectCaps"
                />
                <p v-if="capsLock" class="form-help flex items-center gap-1.5 text-warning">
                    <Info :size="12" aria-hidden="true" />大写锁定已开启
                </p>
            </UiFormField>
            <div v-if="!oidc" class="flex items-center justify-between gap-3">
                <label class="checkbox-field">
                    <UiCheckbox v-model="form.remember" :disabled="pending" />
                    <span>记住用户名</span>
                </label>
            </div>
            <UiButton type="submit" size="lg" block :loading="pending" :disabled="pending">
                登录
                <ArrowRight v-if="!pending" :size="15" aria-hidden="true" />
            </UiButton>
            <div v-if="providers.length" class="space-y-3">
                <div class="flex items-center gap-3">
                    <span class="h-px flex-1 bg-border" />
                    <span class="text-[10.5px] text-muted-foreground">或</span>
                    <span class="h-px flex-1 bg-border" />
                </div>
                <div class="grid gap-2">
                    <UiButton
                        v-for="provider in providers"
                        :key="provider.key"
                        type="button"
                        variant="outline"
                        block
                        class="justify-start gap-3 px-3 py-2.5 text-left"
                        :title="providerHint(provider)"
                        :aria-label="(provider.mode === 'native' ? provider.label + '（小程序）' : provider.label) + '：' + providerHint(provider)"
                        :loading="externalPending === provider.key"
                        :disabled="!!externalPending || pending"
                        @click="externalLogin(provider)"
                    >
                        <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border bg-background">
                            <ExternalProviderIcon :provider="provider.key" :size="17" />
                        </span>
                        <span class="min-w-0 flex-1 text-xs font-semibold">
                            {{ provider.mode === 'native' ? provider.label + '（小程序）' : provider.label }}
                        </span>
                        <ArrowRight
                            v-if="externalPending !== provider.key"
                            :size="14"
                            class="shrink-0 text-muted-foreground"
                            aria-hidden="true"
                        />
                    </UiButton>
                </div>
            </div>
            <div v-if="!oidc" class="auth-links">
                <span>没有账户？</span>
                <RouterLink :to="{ name: 'register' }">使用邀请码</RouterLink>
            </div>
        </form>
    </AuthLayout>
</template>
