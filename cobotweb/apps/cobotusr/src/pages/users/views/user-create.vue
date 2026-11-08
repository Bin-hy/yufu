<script setup lang="ts">
import { reactive, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import * as api from '../api'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import { readAllPages } from '@/composables/use-paged-options'
import { usePageContext } from '@/workspace/page-context'
import { useTabsStore } from '@/pinia/stores/tabs'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'
import DataState from '@/components/workspace/data-state.vue'
const form = reactive({
        username: '',
        nickname: '',
        password: '',
        confirmPassword: '',
        role_id: 0,
    }),
    errors = reactive<Record<string, string>>({})
const router = useRouter(),
    context = usePageContext(),
    tabs = useTabsStore(),
    mutation = useAsyncMutation(),
    { pending, error } = mutation
const draft = useFormDraft(
    '创建用户',
    () => ({ ...form, username: form.username.trim(), nickname: form.nickname.trim() }),
    {
        submitting: pending,
        discard: () =>
            Object.assign(form, {
                username: '',
                nickname: '',
                password: '',
                confirmPassword: '',
                role_id: roles.data.value?.[0]?.id ?? 0,
            }),
    },
)
const roles = useAsyncQuery({
    key: 'create-user-roles',
    active: context.active,
    loader: (signal) =>
        readAllPages(
            (p, s) => api.getModifiableRoles(p, s, { signal, errorPresentation: 'local' }),
            signal,
        ),
})
watch(roles.data, (items) => {
    if (items && !form.role_id) {
        form.role_id = items[0]?.id ?? 0
        draft.rebase({ role_id: form.role_id })
    }
})
function validate() {
    for (const key of Object.keys(errors)) delete errors[key]
    if (!/^[\s\S]{3,20}$/.test(form.username.trim())) errors.username = '用户名应为 3–20 个字符'
    if (form.nickname.trim().length < 2 || form.nickname.trim().length > 20)
        errors.nickname = '昵称应为 2–20 个字符'
    if (form.password.length < 6) errors.password = '密码至少需要 6 位'
    if (form.password !== form.confirmPassword) errors.confirmPassword = '两次密码不一致'
    if (!roles.data.value?.some((role) => role.id === form.role_id))
        errors.role_id = '请选择可用角色'
    if (Object.keys(errors).length)
        void nextTick(() => document.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus())
    return !Object.keys(errors).length
}
watch(
    () => tabs.invalidations['create-user'] ?? 0,
    () => roles.invalidate(),
)
async function submit() {
    if (!validate()) return
    const saved = await mutation.run(
        () =>
            api.createUserWithRole(
                {
                    username: form.username.trim(),
                    nickname: form.nickname.trim(),
                    password: form.password,
                    role_id: form.role_id,
                },
                { errorPresentation: 'local' },
            ),
        async (user) => {
            form.password = ''
            form.confirmPassword = ''
            draft.markSaved()
            tabs.invalidate('user-list')
            emitToast({ type: 'success', message: '已创建用户「' + user.username + '」' })
        },
    )
    if (saved) await router.push({ name: 'user-list' })
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader
            title="创建用户"
            description="为新账户设置基本资料与初始角色。"
            eyebrow="IDENTITY"
        />
        <form class="panel max-w-3xl" @submit.prevent="submit">
            <div class="p-6 grid sm:grid-cols-2 gap-6">
                <div
                    v-for="field in [
                        {
                            key: 'username',
                            label: '用户名',
                            help: '3–20 个字符',
                            type: 'text',
                            autocomplete: 'off',
                        },
                        {
                            key: 'nickname',
                            label: '昵称',
                            help: '2–20 个字符',
                            type: 'text',
                            autocomplete: 'off',
                        },
                        {
                            key: 'password',
                            label: '密码',
                            help: '至少 6 位，建议使用更长的独立密码',
                            type: 'password',
                            autocomplete: 'new-password',
                        },
                        {
                            key: 'confirmPassword',
                            label: '确认密码',
                            help: '再次输入密码',
                            type: 'password',
                            autocomplete: 'new-password',
                        },
                    ]"
                    :key="field.key"
                    class="form-field"
                >
                    <label :for="'create-' + field.key"
                        >{{ field.label }}
                        <span aria-hidden="true" class="text-destructive">*</span></label
                    ><UiInput
                        :id="'create-' + field.key"
                        :model-value="form[field.key as keyof typeof form]"
                        :type="field.type"
                        :autocomplete="field.autocomplete"
                        :disabled="pending"
                        required
                        :aria-invalid="!!errors[field.key]"
                        :aria-describedby="'help-' + field.key"
                        @update:model-value="
                            (value) => {
                                form[field.key] = String(value)
                            }
                        "
                    />
                    <p
                        :id="'help-' + field.key"
                        class="form-help"
                        :class="{ 'text-destructive': errors[field.key] }"
                    >
                        {{ errors[field.key] || field.help }}
                    </p>
                </div>
                <div class="form-field sm:col-span-2">
                    <label for="create-role">角色</label
                    ><DataState
                        :status="roles.status.value"
                        :error="roles.error.value"
                        @retry="roles.refresh"
                        ><UiSelect
                            id="create-role"
                            v-model.number="form.role_id"
                            :disabled="pending || !roles.data.value?.length"
                            ><option
                                v-for="role in roles.data.value"
                                :key="role.id"
                                :value="role.id"
                            >
                                {{ role.name }}
                            </option></UiSelect
                        >
                        <p v-if="!roles.data.value?.length" class="form-help">
                            当前没有可分配的角色。
                        </p>
                        <p v-if="errors.role_id" class="text-destructive text-xs">
                            {{ errors.role_id }}
                        </p></DataState
                    >
                </div>
                <p v-if="error" class="inline-error sm:col-span-2" role="alert">{{ error }}</p>
            </div>
            <footer class="dialog-footer">
                <UiButton
                    variant="outline"
                    :disabled="pending"
                    @click="router.push({ name: 'user-list' })"
                    >取消</UiButton
                ><UiButton
                    type="submit"
                    :disabled="
                        pending || roles.status.value !== 'ready' || !roles.data.value?.length
                    "
                    >{{ pending ? '正在创建…' : '创建用户' }}</UiButton
                >
            </footer>
        </form>
    </section>
</template>
