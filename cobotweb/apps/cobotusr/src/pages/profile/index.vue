<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { getCurrentUser, patchMyProfile } from './api'
import { useUserStore } from '@/pinia/stores/user'
import { usePageContext } from '@/workspace/page-context'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import { logoutOnly } from '@/common/utils/auth'
import { emitToast } from '@/utils/toast'
import { showModal } from '@/utils/modal'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
const router = useRouter(),
    user = useUserStore(),
    context = usePageContext(),
    state = useAsyncQuery({
        key: 'my-profile',
        active: context.active,
        loader: (signal) => getCurrentUser({ signal, errorPresentation: 'local' }),
    }),
    { data, status, error } = state
const nickname = ref(''),
    password = ref(''),
    confirmPassword = ref(''),
    validation = ref('')
const nicknameMutation = useAsyncMutation(),
    passwordMutation = useAsyncMutation(),
    savingNickname = nicknameMutation.pending,
    savingPassword = passwordMutation.pending,
    nicknameError = nicknameMutation.error,
    passwordError = passwordMutation.error
const nicknameDraft = useFormDraft('昵称修改', () => nickname.value.trim(), {
    submitting: savingNickname,
    discard: () => {
        nickname.value = data.value?.nickname ?? ''
    },
})
const passwordDraft = useFormDraft(
    '密码修改',
    () => ({ password: password.value, confirmPassword: confirmPassword.value }),
    {
        submitting: savingPassword,
        discard: () => {
            password.value = ''
            confirmPassword.value = ''
        },
    },
)
watch(data, (info) => {
    if (info) {
        if (!nicknameDraft.dirty.value) {
            nickname.value = info.nickname ?? ''
            nicknameDraft.markSaved()
        } else nicknameDraft.setBaseline(info.nickname ?? '')
    }
})
async function saveNickname() {
    if (!data.value || !nickname.value.trim()) {
        nicknameError.value = '请输入昵称'
        document.getElementById('profile-nickname')?.focus()
        return
    }
    await nicknameMutation.run(
        () =>
            patchMyProfile(
                { user_id: data.value!.id, nickname: nickname.value.trim() },
                { errorPresentation: 'local' },
            ),
        () => {
            if (data.value) data.value.nickname = nickname.value.trim()
            if (user.userInfo)
                user.setUserInfo({ ...user.userInfo, nickname: nickname.value.trim() })
            nicknameDraft.markSaved()
            emitToast({ type: 'success', message: '昵称已保存' })
        },
    )
}
async function savePassword() {
    validation.value = ''
    if (!data.value) return
    if (password.value.length < 6) {
        validation.value = '新密码至少需要 6 位'
        document.getElementById('profile-password')?.focus()
        return
    }
    if (password.value !== confirmPassword.value) {
        validation.value = '两次密码不一致'
        document.getElementById('profile-confirm')?.focus()
        return
    }
    if (
        nicknameDraft.dirty.value &&
        !(await showModal({
            title: '修改密码并退出登录？',
            content: '昵称仍有未保存的修改。密码更新后需要重新登录，昵称草稿将清除。',
            destructive: true,
            confirmText: '修改并退出',
        }))
    )
        return
    const saved = await passwordMutation.run(
        () =>
            patchMyProfile(
                { user_id: data.value!.id, password: password.value },
                { errorPresentation: 'local' },
            ),
        () => {
            password.value = ''
            confirmPassword.value = ''
            passwordDraft.markSaved()
        },
    )
    if (saved) {
        await logoutOnly(router)
        emitToast({ type: 'success', message: '密码已更新，请重新登录' })
    }
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader
            title="个人中心"
            description="更新个人资料，管理账户安全。"
            eyebrow="ACCOUNT"
        /><DataState :status="status" :error="error" @retry="state.refresh"
            ><div class="grid xl:grid-cols-2 gap-6 max-w-5xl">
                <form class="panel" @submit.prevent="saveNickname">
                    <header class="panel-heading">
                        <h2 class="font-semibold">个人资料</h2>
                        <span class="text-xs text-muted-foreground">#{{ data?.id }}</span>
                    </header>
                    <div class="p-6 space-y-5">
                        <div class="flex gap-3 items-center">
                            <span
                                class="w-12 h-12 rounded-full bg-secondary text-primary flex items-center justify-center text-xl font-medium"
                                >{{ (data?.nickname || data?.username || 'U').slice(0, 1) }}</span
                            >
                            <div>
                                <p class="font-medium">{{ data?.nickname || data?.username }}</p>
                                <p class="text-xs text-muted-foreground mt-1">
                                    @{{ data?.username }}
                                </p>
                            </div>
                        </div>
                        <div class="form-field">
                            <label for="profile-nickname">昵称</label
                            ><UiInput
                                id="profile-nickname"
                                v-model="nickname"
                                :disabled="savingNickname || savingPassword"
                                required
                                maxlength="20"
                                :aria-invalid="!!nicknameError"
                                :aria-describedby="
                                    nicknameError ? 'profile-nickname-error' : undefined
                                "
                            />
                        </div>
                        <p
                            v-if="nicknameError"
                            id="profile-nickname-error"
                            role="alert"
                            class="inline-error"
                        >
                            {{ nicknameError }}
                        </p>
                    </div>
                    <footer class="dialog-footer">
                        <UiButton
                            type="submit"
                            :disabled="
                                !nicknameDraft.dirty.value || savingNickname || savingPassword
                            "
                            >{{ savingNickname ? '正在保存…' : '保存昵称' }}</UiButton
                        >
                    </footer>
                </form>
                <form class="panel" @submit.prevent="savePassword">
                    <header class="panel-heading"><h2 class="font-semibold">账户密码</h2></header>
                    <div class="p-6 space-y-5">
                        <p class="form-help">密码更新后，你需要使用新密码重新登录。</p>
                        <div class="form-field">
                            <label for="profile-password">新密码</label
                            ><UiInput
                                id="profile-password"
                                v-model="password"
                                type="password"
                                autocomplete="new-password"
                                :disabled="savingPassword || savingNickname"
                                required
                                minlength="6"
                                :aria-invalid="!!(validation || passwordError)"
                                :aria-describedby="
                                    validation || passwordError
                                        ? 'profile-password-error'
                                        : undefined
                                "
                            />
                        </div>
                        <div class="form-field">
                            <label for="profile-confirm">确认新密码</label
                            ><UiInput
                                id="profile-confirm"
                                v-model="confirmPassword"
                                type="password"
                                autocomplete="new-password"
                                :disabled="savingPassword || savingNickname"
                                required
                                :aria-invalid="!!(validation || passwordError)"
                                :aria-describedby="
                                    validation || passwordError
                                        ? 'profile-password-error'
                                        : undefined
                                "
                            />
                        </div>
                        <p
                            v-if="validation || passwordError"
                            id="profile-password-error"
                            role="alert"
                            class="inline-error"
                        >
                            {{ validation || passwordError }}
                        </p>
                    </div>
                    <footer class="dialog-footer">
                        <UiButton
                            type="submit"
                            :disabled="
                                savingPassword || savingNickname || !passwordDraft.dirty.value
                            "
                            >{{ savingPassword ? '正在更新…' : '更新密码' }}</UiButton
                        >
                    </footer>
                </form>
            </div></DataState
        >
    </section>
</template>
