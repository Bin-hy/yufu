<script setup lang="ts">
import { computed, reactive, ref, onScopeDispose, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { ArrowRight, Info } from '@lucide/vue'
import { centerLogin } from '@/pages/account/api'
import { useUserStore } from '@/pinia/stores/user'
import { useTabsStore } from '@/pinia/stores/tabs'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { setRememberUser, removeRememberUser, getRememberUser } from '@/common/cache/localstorage'
import { resetSession, sessionOwner } from '@/workspace/session'
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
        :title="restoreUnavailable ? '暂时无法验证登录状态' : oidc ? '登录以继续授权' : '欢迎回来'"
        :description="
            restoreUnavailable
                ? '身份服务暂不可用，还不能判断原会话是否过期。恢复连接后可重试。'
                : oidc
                ? '使用企业统一账户登录，确认应用请求的访问范围。'
                : '登录账户，进入身份与权限工作台。'
        "
    >
        <div v-if="restoreUnavailable" class="space-y-4" role="status">
            <p class="form-alert">{{ user.identityError || '正在重新验证登录状态…' }}</p>
            <UiButton block :loading="retrying" :disabled="retrying" @click="retrySession">重新验证登录状态</UiButton>
        </div>
        <form v-else class="space-y-4" novalidate @submit.prevent="submit">
            <p v-if="validation || error" class="form-alert" role="alert">
                {{ validation || error }}
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
            <div class="flex items-center justify-between gap-3">
                <label v-if="!oidc" class="checkbox-field">
                    <UiCheckbox v-model="form.remember" :disabled="pending" />
                    <span>记住用户名</span>
                </label>
                <span v-else class="text-[11px] text-muted-foreground">受企业安全策略保护</span>
                <span class="text-[11px] text-muted-foreground">忘记密码请联系管理员</span>
            </div>
            <UiButton type="submit" size="lg" block :loading="pending" :disabled="pending">
                登录
                <ArrowRight v-if="!pending" :size="15" aria-hidden="true" />
            </UiButton>
            <div v-if="!oidc" class="auth-links">
                <span>还没有账户？</span>
                <RouterLink :to="{ name: 'register' }">凭企业邀请创建账户</RouterLink>
            </div>
            <p class="text-center text-[10.5px] leading-5 text-muted-foreground/80">
                继续即表示你同意企业的身份认证与访问控制策略。
            </p>
        </form>
    </AuthLayout>
</template>
