<script setup lang="ts">
defineOptions({ name: "AccountProfile" })
import { reactive, ref, watch, onScopeDispose, onDeactivated } from 'vue'
import { useRouter } from 'vue-router'
import { useUserStore } from '@/pinia/stores/user'
import { updateProfile, changePassword } from '../api'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { onSessionReset } from '@/workspace/session'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
const user = useUserStore(), router = useRouter()
const profile = reactive({ nickname: '', email: '' }), passwords = reactive({ old: '', next: '', confirm: '' })
const mutation = useAsyncMutation(), { pending, error } = mutation, validation = ref('')
watch(() => user.centerSession, value => { profile.nickname = value?.nickname ?? ''; profile.email = value?.email ?? '' }, { immediate: true })
const clear = () => { passwords.old = ''; passwords.next = ''; passwords.confirm = '' }
onScopeDispose(onSessionReset(clear)); onScopeDispose(clear)
onDeactivated(clear)
async function save() {
    await mutation.run(() => updateProfile({ nickname: profile.nickname.trim(), email: profile.email.trim() || null }), async () => {
        await user.ensureSession(); emitToast({ type: 'success', message: '账户资料已更新' })
    })
}
async function password() {
    validation.value = ''
    if (!passwords.old || passwords.next.length < 8 || passwords.next !== passwords.confirm) { validation.value = '请填写旧密码，新密码至少 8 位且两次输入一致'; return }
    const saved = await mutation.run(() => changePassword({ old_password: passwords.old, new_password: passwords.next }))
    clear()
    if (saved) { user.logout(); emitToast({ type: 'success', message: '密码已修改，原登录会话已撤销，请重新登录' }); await router.replace('/login') }
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader title="账户中心" eyebrow="ACCOUNT" description="使用同一个企业账户登录获准接入的应用。" />
        <p v-if="error || validation" role="alert" class="inline-error mb-4">{{ error || validation }}</p>
        <div class="grid gap-6 lg:grid-cols-2">
            <section class="rounded-xl border bg-card p-6">
                <h2 class="mb-4 font-semibold">账户资料</h2>
                <p class="mb-4 text-sm text-muted-foreground">用户名：{{ user.centerSession?.username }} · {{ user.centerSession?.person_kind === 'employee' ? '企业员工' : '受邀合作方' }}</p>
                <form class="space-y-4" @submit.prevent="save">
                    <div class="form-field"><label for="account-nickname">显示名称</label><UiInput id="account-nickname" v-model="profile.nickname" maxlength="200" :disabled="pending" required /></div>
                    <div class="form-field"><label for="account-email">邮箱（可选）</label><UiInput id="account-email" v-model="profile.email" type="email" maxlength="254" :disabled="pending" autocomplete="email" /><p class="text-xs text-muted-foreground">邮箱尚未验证；更改邮箱不会自动标记为已验证。</p></div>
                    <UiButton type="submit" :disabled="pending">保存资料</UiButton>
                </form>
            </section>
            <section class="rounded-xl border bg-card p-6">
                <h2 class="mb-4 font-semibold">修改密码</h2>
                <p class="mb-4 text-sm text-muted-foreground">成功修改后会撤销全部中心与应用授权，需要重新认证。</p>
                <form class="space-y-4" @submit.prevent="password">
                    <div class="form-field"><label for="old-password">当前密码</label><UiInput id="old-password" v-model="passwords.old" type="password" autocomplete="current-password" :disabled="pending" required /></div>
                    <div class="form-field"><label for="new-password">新密码</label><UiInput id="new-password" v-model="passwords.next" type="password" autocomplete="new-password" :disabled="pending" required /></div>
                    <div class="form-field"><label for="confirm-password">确认新密码</label><UiInput id="confirm-password" v-model="passwords.confirm" type="password" autocomplete="new-password" :disabled="pending" required /></div>
                    <UiButton type="submit" :disabled="pending">修改并退出原会话</UiButton>
                </form>
            </section>
        </div>
    </div>
</template>
