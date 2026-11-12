<script setup lang="ts">
defineOptions({ name: 'AccountProfile' })
import { computed, onDeactivated, onScopeDispose, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ArrowRight, BadgeCheck, LockKeyhole, Mail, ShieldCheck, UserRound } from '@lucide/vue'
import { useUserStore } from '@/pinia/stores/user'
import { updateProfile, changePassword } from '../api'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { onSessionReset } from '@/workspace/session'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
const user = useUserStore(),
    router = useRouter()
const profile = reactive({ nickname: '', email: '' }),
    passwords = reactive({ old: '', next: '', confirm: '' })
const mutation = useAsyncMutation(),
    { pending, error } = mutation,
    validation = ref('')
watch(
    () => user.centerSession,
    (value) => {
        profile.nickname = value?.nickname ?? ''
        profile.email = value?.email ?? ''
    },
    { immediate: true },
)
const displayName = computed(
    () => user.centerSession?.nickname || user.centerSession?.username || '账户',
)
const avatar = computed(() => displayName.value.slice(0, 1).toUpperCase())
const passwordStrength = computed(() => {
    const value = passwords.next
    if (!value) return { label: '至少 8 位字符', tone: 'muted', width: '0%' }
    let score = 0
    if (value.length >= 8) score++
    if (/[A-Z]/.test(value) && /[a-z]/.test(value)) score++
    if (/\d/.test(value)) score++
    if (/[^A-Za-z0-9]/.test(value)) score++
    if (score <= 1) return { label: '较弱', tone: 'danger', width: '28%' }
    if (score === 2) return { label: '一般', tone: 'warning', width: '56%' }
    if (score === 3) return { label: '良好', tone: 'info', width: '78%' }
    return { label: '很强', tone: 'success', width: '100%' }
})
const strengthBarClass = computed(
    () =>
        ({
            danger: 'bg-danger',
            warning: 'bg-warning',
            info: 'bg-info',
            success: 'bg-success',
            muted: 'bg-muted',
        })[passwordStrength.value.tone] ?? 'bg-muted',
)
const passwordsMatch = computed(
    () => !!passwords.confirm && passwords.next === passwords.confirm,
)
const clear = () => {
    passwords.old = ''
    passwords.next = ''
    passwords.confirm = ''
}
onScopeDispose(onSessionReset(clear))
onScopeDispose(clear)
onDeactivated(clear)
async function save() {
    if (pending.value) return
    validation.value = ''
    if (!profile.nickname.trim()) {
        validation.value = '请输入显示名称'
        document.getElementById('account-nickname')?.focus()
        return
    }
    await mutation.run(
        () =>
            updateProfile({
                nickname: profile.nickname.trim(),
                email: profile.email.trim() || null,
            }),
        async () => {
            await user.ensureSession()
            emitToast({ type: 'success', message: '账户资料已更新' })
        },
    )
}
async function password() {
    if (pending.value) return
    validation.value = ''
    if (!passwords.old || passwords.next.length < 8 || passwords.next !== passwords.confirm) {
        validation.value = '请填写旧密码，新密码至少 8 位且两次输入一致'
        document
            .getElementById(!passwords.old ? 'old-password' : 'new-password')
            ?.focus()
        return
    }
    const saved = await mutation.run(() =>
        changePassword({ old_password: passwords.old, new_password: passwords.next }),
    )
    clear()
    if (saved) {
        user.logout()
        emitToast({ type: 'success', message: '密码已修改，原登录会话已撤销，请重新登录' })
        await router.replace('/login')
    }
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="账户中心"
            eyebrow="ACCOUNT"
            description="使用同一个企业账户登录获准接入的应用。资料与密码修改会记录在身份审计中。"
        >
            <UiButton variant="outline" @click="router.push('/account/sessions')">
                <ShieldCheck :size="14" aria-hidden="true" />登录会话
            </UiButton>
        </PageHeader>

        <p v-if="error || validation" role="alert" class="form-alert mb-4">
            {{ error || validation }}
        </p>

        <div class="content-grid content-grid-sidebar">
            <div class="section-stack">
                <section class="panel">
                    <header class="panel-heading">
                        <div>
                            <h2>账户资料</h2>
                            <p>这些信息会作为应用登录时的账户资料。</p>
                        </div>
                        <span class="status-badge status-brand">统一身份</span>
                    </header>
                    <form class="surface-body space-y-5" novalidate @submit.prevent="save">
                        <div class="grid gap-4 md:grid-cols-2">
                            <UiFormField
                                id="account-nickname"
                                label="显示名称"
                                required
                                hint="用于应用内展示，可随时修改。"
                            >
                                <UiInput
                                    id="account-nickname"
                                    v-model="profile.nickname"
                                    maxlength="200"
                                    required
                                    :disabled="pending"
                                    :invalid="!profile.nickname.trim() && !!validation"
                                    autocomplete="nickname"
                                    placeholder="请输入显示名称"
                                />
                            </UiFormField>
                            <UiFormField
                                id="account-email"
                                label="邮箱"
                                optional
                                hint="邮箱尚未验证；更改邮箱不会自动标记为已验证。"
                            >
                                <UiInput
                                    id="account-email"
                                    v-model="profile.email"
                                    type="email"
                                    maxlength="254"
                                    :disabled="pending"
                                    autocomplete="email"
                                    placeholder="name@company.com"
                                />
                            </UiFormField>
                        </div>
                        <div class="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4">
                            <p class="form-help">用户名 <span class="mono">{{ user.centerSession?.username }}</span> 不可修改。</p>
                            <UiButton type="submit" :loading="pending" :disabled="pending">
                                保存资料
                            </UiButton>
                        </div>
                    </form>
                </section>

                <section class="panel">
                    <header class="panel-heading">
                        <div>
                            <h2>修改密码</h2>
                            <p>成功修改后会撤销全部中心与应用授权，需要重新认证。</p>
                        </div>
                        <LockKeyhole :size="18" class="text-muted-foreground" aria-hidden="true" />
                    </header>
                    <form class="surface-body space-y-4" novalidate @submit.prevent="password">
                        <UiFormField id="old-password" label="当前密码" required>
                            <UiInput
                                id="old-password"
                                v-model="passwords.old"
                                type="password"
                                autocomplete="current-password"
                                :disabled="pending"
                                required
                                placeholder="请输入当前密码"
                            />
                        </UiFormField>
                        <div class="grid gap-4 md:grid-cols-2">
                            <UiFormField id="new-password" label="新密码" required>
                                <UiInput
                                    id="new-password"
                                    v-model="passwords.next"
                                    type="password"
                                    autocomplete="new-password"
                                    :disabled="pending"
                                    required
                                    minlength="8"
                                    placeholder="至少 8 位字符"
                                />
                            </UiFormField>
                            <UiFormField id="confirm-password" label="确认新密码" required>
                                <UiInput
                                    id="confirm-password"
                                    v-model="passwords.confirm"
                                    type="password"
                                    autocomplete="new-password"
                                    :disabled="pending"
                                    required
                                    placeholder="再次输入新密码"
                                />
                            </UiFormField>
                        </div>
                        <div v-if="passwords.next" class="space-y-2">
                            <div class="h-1.5 overflow-hidden rounded-full bg-muted">
                                <div
                                    class="h-full rounded-full transition-all duration-300"
                                    :class="strengthBarClass"
                                    :style="{ width: passwordStrength.width }"
                                />
                            </div>
                            <div class="flex items-center justify-between gap-3 text-[10.5px] text-muted-foreground">
                                <span>密码强度：{{ passwordStrength.label }}</span>
                                <span v-if="passwords.confirm" :class="passwordsMatch ? 'text-success' : 'text-destructive'">
                                    {{ passwordsMatch ? '两次输入一致' : '两次输入不一致' }}
                                </span>
                            </div>
                        </div>
                        <div class="flex justify-end border-t border-border pt-4">
                            <UiButton type="submit" :loading="pending" :disabled="pending">
                                修改并退出原会话
                            </UiButton>
                        </div>
                    </form>
                </section>
            </div>

            <aside class="section-stack sticky-column">
                <section class="surface p-5">
                    <div class="flex items-center gap-3">
                        <span class="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary text-lg font-semibold text-primary-foreground shadow-sm">
                            {{ avatar }}
                        </span>
                        <div class="min-w-0">
                            <p class="truncate text-sm font-semibold">{{ displayName }}</p>
                            <p class="mt-0.5 truncate text-[11px] text-muted-foreground">@{{ user.centerSession?.username }}</p>
                        </div>
                    </div>
                    <dl class="mt-5 space-y-3 border-t border-border pt-4 text-[11.5px]">
                        <div class="flex items-center justify-between gap-3">
                            <dt class="text-muted-foreground">账户类型</dt>
                            <dd>{{ user.centerSession?.person_kind === 'employee' ? '企业员工' : '受邀合作方' }}</dd>
                        </div>
                        <div class="flex items-center justify-between gap-3">
                            <dt class="text-muted-foreground">开发者资格</dt>
                            <dd>{{ user.centerSession?.developer_enabled ? '已开通' : '未开通' }}</dd>
                        </div>
                        <div class="flex items-center justify-between gap-3">
                            <dt class="text-muted-foreground">管理员</dt>
                            <dd>{{ user.centerSession?.administrator ? '是' : '否' }}</dd>
                        </div>
                    </dl>
                </section>
                <section class="surface p-5">
                    <p class="flex items-center gap-2 text-xs font-semibold"><BadgeCheck :size="15" class="text-success" aria-hidden="true" />账户安全建议</p>
                    <ul class="mt-3 space-y-2 text-[11px] leading-5 text-muted-foreground">
                        <li class="flex gap-2"><Mail :size="12" class="mt-1 shrink-0" aria-hidden="true" />定期确认邮箱可用，便于接收安全通知。</li>
                        <li class="flex gap-2"><UserRound :size="12" class="mt-1 shrink-0" aria-hidden="true" />不要在共享账户中保存个人资料。</li>
                        <li class="flex gap-2"><ArrowRight :size="12" class="mt-1 shrink-0" aria-hidden="true" />发现异常登录时，立即在登录会话中撤销。</li>
                    </ul>
                </section>
            </aside>
        </div>
    </div>
</template>
