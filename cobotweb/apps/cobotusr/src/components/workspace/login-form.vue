<script setup lang="ts">
import { reactive, ref, onScopeDispose } from 'vue'
import { useRouter, useRoute } from 'vue-router'
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
    mutation = useAsyncMutation(),
    { pending, error } = mutation
onScopeDispose(() => { form.password = '' })
async function submit() {
    if (pending.value) return
    validation.value = ''
    if (!form.username.trim() || !form.password) {
        validation.value = '请输入用户名和密码'
        return
    }
    resetSession()
    const saved = await mutation.run(
        () =>
            centerLogin({ username: form.username.trim(), password: form.password,
                flow_id: props.flowId || (typeof route.query.flow_id === 'string' ? route.query.flow_id : undefined) }),
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
            const flow = props.flowId || (typeof route.query.flow_id === 'string' ? route.query.flow_id : '')
            window.location.assign(router.resolve({ name: 'oidc-consent', query: { flow_id: flow } }).href)
        }
        else {
            const path =
                typeof route.query.redirect === 'string' ? route.query.redirect : '/account'
            const target =
                path.startsWith('/') &&
                !path.startsWith('//') &&
                router.resolve(path).meta.workspace
                    ? path
                    : '/account'
            await router.replace(target)
        }
    }
}
</script>
<template>
    <AuthLayout
        :title="oidc ? '登录以继续授权' : '欢迎回来'"
        :description="
            oidc ? '登录平台账户，确认应用请求的访问权限。' : '登录账户，进入身份与权限工作台。'
        "
        ><form class="space-y-5" @submit.prevent="submit">
            <p v-if="validation || error" class="inline-error" role="alert">
                {{ validation || error }}
            </p>
            <div class="form-field">
                <label for="login-username">用户名</label
                ><UiInput
                    id="login-username"
                    v-model="form.username"
                    autocomplete="username"
                    :disabled="pending"
                    required
                    autofocus
                />
            </div>
            <div class="form-field">
                <label for="login-password">密码</label
                ><UiInput
                    id="login-password"
                    v-model="form.password"
                    type="password"
                    autocomplete="current-password"
                    :disabled="pending"
                    required
                />
            </div>
            <label v-if="!oidc" class="flex gap-2 items-center text-xs text-muted-foreground"
                ><UiCheckbox v-model="form.remember" :disabled="pending" />记住用户名</label
            ><UiButton type="submit" size="lg" class="w-full" :disabled="pending">{{
                pending ? '正在登录…' : '登录'
            }}</UiButton>
            <p v-if="!oidc" class="text-center text-xs text-muted-foreground">
                还没有账户？<RouterLink
                    :to="{ name: 'register' }"
                    class="text-primary underline underline-offset-4"
                    >凭企业邀请创建账户</RouterLink
                >
            </p>
        </form></AuthLayout
    >
</template>
