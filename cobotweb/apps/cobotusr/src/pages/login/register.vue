<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { createUser } from '@/pages/users/api'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import { emitToast } from '@/utils/toast'
import AuthLayout from '@/layouts/auth-layout.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
const router = useRouter(),
    form = reactive({
        username: '',
        nickname: '',
        password: '',
        confirmPassword: '',
        agree: false,
    }),
    validation = ref(''),
    mutation = useAsyncMutation(),
    { pending, error } = mutation
const draft = useFormDraft('注册账户', () => form, {
    submitting: pending,
    discard: () =>
        Object.assign(form, {
            username: '',
            nickname: '',
            password: '',
            confirmPassword: '',
            agree: false,
        }),
})
const fields = [
    { key: 'username' as const, label: '用户名', type: 'text', autocomplete: 'username' },
    { key: 'nickname' as const, label: '昵称', type: 'text', autocomplete: 'nickname' },
    { key: 'password' as const, label: '密码', type: 'password', autocomplete: 'new-password' },
    {
        key: 'confirmPassword' as const,
        label: '确认密码',
        type: 'password',
        autocomplete: 'new-password',
    },
]
async function submit() {
    validation.value = ''
    if (!form.username.trim() || !form.nickname.trim()) {
        validation.value = '请填写用户名和昵称'
        return
    }
    if (form.password.length < 6 || form.password !== form.confirmPassword) {
        validation.value = '密码至少需要 6 位，两次输入应一致'
        return
    }
    if (!form.agree) {
        validation.value = '请先同意用户协议与隐私政策'
        return
    }
    const saved = await mutation.run(
        () =>
            createUser(
                {
                    username: form.username.trim(),
                    nickname: form.nickname.trim(),
                    password: form.password,
                },
                { errorPresentation: 'local' },
            ),
        () => {
            form.password = ''
            form.confirmPassword = ''
            draft.markSaved()
            emitToast({ type: 'success', message: '账号已创建，请登录' })
        },
    )
    if (saved) await router.replace({ name: 'login' })
}
</script>
<template>
    <AuthLayout title="创建账户" description="填写账户资料，开始使用平台服务。"
        ><form class="space-y-4" @submit.prevent="submit">
            <div v-for="field in fields" :key="field.key" class="form-field">
                <label :for="'register-' + field.key">{{ field.label }}</label
                ><UiInput
                    :id="'register-' + field.key"
                    v-model="form[field.key]"
                    :type="field.type"
                    :autocomplete="field.autocomplete"
                    :disabled="pending"
                    required
                />
            </div>
            <label class="flex gap-2 items-start text-xs text-muted-foreground leading-5"
                ><UiCheckbox
                    v-model="form.agree"
                    :disabled="pending"
                />我已阅读并同意用户协议与隐私政策</label
            >
            <p v-if="validation || error" class="inline-error" role="alert">
                {{ validation || error }}
            </p>
            <UiButton type="submit" class="w-full" size="lg" :disabled="pending">{{
                pending ? '正在注册…' : '创建账户'
            }}</UiButton>
            <p class="text-xs text-center text-muted-foreground">
                已有账户？<RouterLink
                    :to="{ name: 'login' }"
                    class="text-primary underline underline-offset-4"
                    >返回登录</RouterLink
                >
            </p>
        </form></AuthLayout
    >
</template>
