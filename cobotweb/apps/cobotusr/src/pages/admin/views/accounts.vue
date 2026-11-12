<script setup lang="ts">
defineOptions({ name: 'AdminAccounts' })
import { computed, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Copy, Link2, Search, UserPlus, Users } from '@lucide/vue'
import { listAccounts, setAccount, issueInvitation, issueReset, revokeAction, setCandidate } from '../api'
import type { EnterpriseAccount, IssuedAction } from '../api/type'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult } from '@/composables/use-ephemeral-result'
import { useUserStore } from '@/pinia/stores/user'
import { emitToast } from '@/utils/toast'
import { showModal } from '@/utils/modal'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
const list = useCursorPage('enterprise-accounts', listAccounts),
    { data, status, error, refreshing } = list
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation,
    user = useUserStore(),
    router = useRouter()
const opened = ref(false),
    query = ref(''),
    statusFilter = ref<'all' | 'active' | 'disabled'>('all')
const form = reactive({
    purpose: 'onboarding' as 'onboarding' | 'qualification',
    target: '' as number | '',
    person_kind: 'employee' as 'employee' | 'collaborator',
    developer_grant: false,
})
const candidate = reactive({
    requester: '' as number | '',
    target: '' as number | '',
    allowed: true,
})
const issued = useEphemeralResult<{ action: IssuedAction; reset: boolean }>(),
    result = issued.value
const filtered = computed(() => {
    const keyword = query.value.trim().toLowerCase()
    return (data.value?.items ?? []).filter((account) => {
        const matchesStatus =
            statusFilter.value === 'all' ||
            (statusFilter.value === 'active' ? account.enabled : !account.enabled)
        const matchesQuery =
            !keyword ||
            account.username.toLowerCase().includes(keyword) ||
            (account.nickname ?? '').toLowerCase().includes(keyword) ||
            String(account.user_id).includes(keyword)
        return matchesStatus && matchesQuery
    })
})
const link = computed(() => {
    if (!result.value) return ''
    return (
        new URL(router.resolve({ name: 'account-action' }).href, window.location.origin).href +
        '#kind=' +
        (result.value.reset ? 'password_reset' : 'invitation') +
        '&secret=' +
        encodeURIComponent(result.value.action.secret)
    )
})
async function copyLink() {
    try {
        await navigator.clipboard.writeText(link.value)
        emitToast({ type: 'success', message: '链接已复制到剪贴板' })
    } catch {
        emitToast({ type: 'error', message: '无法访问剪贴板，请手动选择并复制链接' })
    }
}
async function invite() {
    if (form.purpose === 'qualification' && !form.target) {
        emitToast({ type: 'warning', message: '资格邀请必须指定已有账户 ID' })
        document.getElementById('invitation-target')?.focus()
        return
    }
    await mutation.run(
        () =>
            issueInvitation({
                purpose: form.purpose,
                target_user_id: form.target || null,
                person_kind: form.person_kind,
                developer_grant: form.developer_grant,
            }),
        (value) => {
            result.value = { action: value, reset: false }
            opened.value = false
            form.target = ''
            form.developer_grant = false
            emitToast({ type: 'success', message: '一次性邀请已签发' })
        },
    )
}
async function reset(account: EnterpriseAccount) {
    await mutation.run(
        () => issueReset(account.user_id),
        (value) => {
            result.value = { action: value, reset: true }
            emitToast({ type: 'success', message: '密码重置链接已签发' })
        },
    )
}
async function change(account: EnterpriseAccount, field: 'enabled' | 'developer') {
    const disabling = field === 'enabled' && account.enabled
    const revoking = field === 'developer' && account.developer_enabled
    if (disabling || revoking) {
        const confirmed = await showModal({
            title: disabling ? '停用账户' : '撤销开发者资格',
            content: disabling
                ? `停用 ${account.nickname || account.username} 后将撤销其全部登录会话与应用授权。`
                : `撤销 ${account.nickname || account.username} 的开发者资格后，其将无法进入开发者控制台。`,
            confirmText: disabling ? '停用并撤销登录' : '撤销开发者资格',
            destructive: true,
        })
        if (!confirmed) return
    }
    await mutation.run(
        () =>
            setAccount(
                account.user_id,
                field,
                !(field === 'developer' ? account.developer_enabled : account.enabled),
                account.revision,
            ),
        async () => {
            await list.refresh()
            if (account.user_id === user.centerSession?.user_id) {
                await user.ensureSession()
                if (!user.centerSession) await router.replace('/login')
            }
            emitToast({ type: 'success', message: '账户状态已更新' })
        },
    )
}
async function revoke() {
    if (!result.value) return
    const confirmed = await showModal({
        title: '撤销一次性链接',
        content: '撤销后，已发出的链接将立即失效，接收人无法继续使用。',
        confirmText: '撤销链接',
        destructive: true,
    })
    if (!confirmed) return
    await mutation.run(
        () => revokeAction(result.value!.action.id, result.value!.reset),
        () => {
            issued.clear()
            emitToast({ type: 'success', message: '链接已撤销' })
        },
    )
}
async function allowCandidate() {
    if (!candidate.requester || !candidate.target) return
    await mutation.run(
        () => setCandidate(Number(candidate.requester), Number(candidate.target), candidate.allowed),
        () => emitToast({ type: 'success', message: '候选可见性已更新' }),
    )
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="企业账户与邀请"
            eyebrow="DIRECTORY"
            description="受控邀请创建或扩充账户资格；开发者资格不自动授予生产应用访问。"
        >
            <UiButton @click="opened = true">
                <UserPlus :size="14" aria-hidden="true" />签发邀请
            </UiButton>
        </PageHeader>

        <p v-if="writeError" class="form-alert mb-4" role="alert">{{ writeError }}</p>

        <section v-if="result" class="panel mb-5 border-warning/30">
            <header class="panel-heading bg-warning/[0.04]">
                <div>
                    <h2 class="flex items-center gap-2">
                        <Link2 :size="15" class="text-warning" aria-hidden="true" />
                        {{ result.reset ? '密码重置链接' : '企业邀请链接' }}（仅此次展示）
                    </h2>
                    <p>到期 {{ new Date(result.action.expires_at * 1000).toLocaleString('zh-CN') }}</p>
                </div>
                <span class="status-badge status-warning">一次性</span>
            </header>
            <div class="surface-body space-y-3">
                <div class="copy-field">
                    <code>{{ link }}</code>
                    <button type="button" class="icon-button" aria-label="复制链接" title="复制链接" @click="copyLink">
                        <Copy :size="14" aria-hidden="true" />
                    </button>
                </div>
                <p class="form-help">通过企业认可的渠道交给指定接收人。离开页面后链接会从界面清除，但撤销前仍然有效。</p>
                <div class="flex flex-wrap gap-2">
                    <UiButton variant="outline" :disabled="pending" :loading="pending" @click="revoke">提前撤销此链接</UiButton>
                    <UiButton variant="ghost" @click="issued.clear()">隐藏链接</UiButton>
                </div>
            </div>
        </section>

        <section class="panel">
            <div class="table-toolbar">
                <div class="search-box">
                    <Search :size="15" aria-hidden="true" />
                    <UiInput v-model="query" clearable placeholder="搜索账户名、用户名或 ID…" aria-label="搜索企业账户" />
                </div>
                <div class="filter-chips" role="group" aria-label="账户状态筛选">
                    <button class="filter-chip" type="button" :aria-pressed="statusFilter === 'all'" @click="statusFilter = 'all'">全部</button>
                    <button class="filter-chip" type="button" :aria-pressed="statusFilter === 'active'" @click="statusFilter = 'active'">启用</button>
                    <button class="filter-chip" type="button" :aria-pressed="statusFilter === 'disabled'" @click="statusFilter = 'disabled'">停用</button>
                </div>
                <span class="toolbar-spacer text-[11px] text-muted-foreground">{{ filtered.length }} 个账户</span>
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!data?.items.length"
                empty-title="还没有企业账户"
                empty-description="签发第一张企业邀请，受控创建账户。"
                :skeleton-rows="3"
                @retry="list.refresh()"
            >
                <div class="table-scroll desktop-table">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th>账户</th>
                                <th>类型</th>
                                <th>状态</th>
                                <th>开发者资格</th>
                                <th class="text-right">操作</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="account in filtered" :key="account.user_id">
                                <td>
                                    <div class="flex items-center gap-3">
                                        <span class="list-icon !h-9 !w-9 !rounded-lg text-[11px] font-semibold">
                                            {{ (account.nickname || account.username).slice(0, 1).toUpperCase() }}
                                        </span>
                                        <div>
                                            <p class="cell-primary">{{ account.nickname || account.username }}</p>
                                            <p class="cell-muted mt-0.5">{{ account.username }} · #{{ account.user_id }}</p>
                                        </div>
                                    </div>
                                </td>
                                <td>{{ account.person_kind === 'employee' ? '企业员工' : '合作方' }}</td>
                                <td>
                                    <span class="status-badge" :class="account.enabled ? 'status-success' : 'status-muted'">
                                        {{ account.enabled ? '启用' : '停用' }}
                                    </span>
                                </td>
                                <td>
                                    <span class="status-badge" :class="account.developer_enabled ? 'status-info' : 'status-muted'">
                                        {{ account.developer_enabled ? '已开通' : '未开通' }}
                                    </span>
                                </td>
                                <td class="cell-actions">
                                    <div class="table-actions">
                                        <UiButton variant="ghost" size="sm" :disabled="pending" @click="reset(account)">重置密码</UiButton>
                                        <UiButton variant="ghost" size="sm" :disabled="pending" @click="change(account, 'developer')">
                                            {{ account.developer_enabled ? '撤销开发者' : '授予开发者' }}
                                        </UiButton>
                                        <UiButton
                                            :variant="account.enabled ? 'outline' : 'default'"
                                            :class="account.enabled ? '!border-destructive/25 !text-destructive hover:!bg-destructive/5' : ''"
                                            size="sm"
                                            :disabled="pending"
                                            @click="change(account, 'enabled')"
                                        >
                                            {{ account.enabled ? '停用' : '启用' }}
                                        </UiButton>
                                    </div>
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="mobile-list">
                    <article v-for="account in filtered" :key="account.user_id" class="list-card">
                        <div class="flex items-start gap-3">
                            <span class="list-icon">{{ (account.nickname || account.username).slice(0, 1).toUpperCase() }}</span>
                            <div class="min-w-0">
                                <div class="flex flex-wrap items-center gap-2">
                                    <h3 class="list-title">{{ account.nickname || account.username }}</h3>
                                    <span class="status-badge" :class="account.enabled ? 'status-success' : 'status-muted'">{{ account.enabled ? '启用' : '停用' }}</span>
                                </div>
                                <p class="list-meta">{{ account.username }} · #{{ account.user_id }} · {{ account.person_kind === 'employee' ? '企业员工' : '合作方' }}</p>
                            </div>
                        </div>
                        <div class="list-row-actions">
                            <UiButton variant="outline" size="sm" :disabled="pending" @click="reset(account)">重置密码</UiButton>
                            <UiButton variant="outline" size="sm" :disabled="pending" @click="change(account, 'developer')">{{ account.developer_enabled ? '撤销开发者' : '授予开发者' }}</UiButton>
                            <UiButton :variant="account.enabled ? 'outline' : 'default'" :class="account.enabled ? '!border-destructive/25 !text-destructive hover:!bg-destructive/5' : ''" size="sm" :disabled="pending" @click="change(account, 'enabled')">{{ account.enabled ? '停用' : '启用' }}</UiButton>
                        </div>
                    </article>
                </div>
                <p v-if="!filtered.length" class="empty-filter">没有匹配当前筛选条件的账户。</p>
                <div v-if="data?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多账户</UiButton>
                </div>
            </DataState>
        </section>

        <section class="panel mt-5">
            <header class="panel-heading">
                <div><h2>合作方候选可见性</h2><p>只为明确合作关系授予候选可见性，不开放企业人员目录。</p></div>
                <Users :size="18" class="text-muted-foreground" aria-hidden="true" />
            </header>
            <form class="surface-body flex flex-wrap items-end gap-3" @submit.prevent="allowCandidate">
                <UiFormField id="candidate-requester" label="请求方账户 ID" class="w-40">
                    <UiInput id="candidate-requester" v-model.number="candidate.requester" type="number" min="1" required class="!min-h-9" />
                </UiFormField>
                <UiFormField id="candidate-target" label="候选账户 ID" class="w-40">
                    <UiInput id="candidate-target" v-model.number="candidate.target" type="number" min="1" required class="!min-h-9" />
                </UiFormField>
                <label class="checkbox-field mb-2">
                    <input v-model="candidate.allowed" type="checkbox" />
                    <span>允许可见</span>
                </label>
                <UiButton type="submit" :disabled="pending" :loading="pending">更新候选范围</UiButton>
            </form>
        </section>

        <UiDialog
            v-model:open="opened"
            title="签发企业邀请"
            description="邀请链接一次性有效，接收人可创建新账户或扩充已有账户资格。"
            confirm-text="签发一次性邀请"
            :busy="pending"
            :error="writeError"
            size="lg"
            @confirm="invite"
        >
            <div class="grid gap-4 md:grid-cols-2">
                <UiFormField id="invitation-purpose" label="邀请用途">
                    <UiSelect id="invitation-purpose" v-model="form.purpose">
                        <option value="onboarding">开户邀请（可接受到已有账户）</option>
                        <option value="qualification">已有账户资格邀请</option>
                    </UiSelect>
                </UiFormField>
                <UiFormField id="invitation-person" label="人员类型">
                    <UiSelect id="invitation-person" v-model="form.person_kind">
                        <option value="employee">企业员工</option>
                        <option value="collaborator">受邀合作方</option>
                    </UiSelect>
                </UiFormField>
                <UiFormField
                    id="invitation-target"
                    label="限定接收账户 ID"
                    :optional="form.purpose === 'onboarding'"
                    :required="form.purpose === 'qualification'"
                    hint="开户邀请可留空；资格邀请必须指定已有账户。"
                >
                    <UiInput id="invitation-target" v-model.number="form.target" type="number" min="0" :required="form.purpose === 'qualification'" />
                </UiFormField>
                <label class="checkbox-field md:mt-7">
                    <input v-model="form.developer_grant" type="checkbox" />
                    <span>明确授予开发者资格</span>
                </label>
            </div>
        </UiDialog>
    </div>
</template>
