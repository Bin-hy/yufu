<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Copy, KeyRound, RefreshCw, ShieldCheck, Users } from '@lucide/vue'
import { usePageContext } from '@/workspace/page-context'
import { useUserStore } from '@/pinia/stores/user'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult, useOperationKey } from '@/composables/use-ephemeral-result'
import {
    getApp,
    changeApp,
    getReaders,
    changeReader,
    getApproval,
    searchCandidates,
    getAppDeliveries,
    getProviderMetadata,
} from '../api'
import { getAssignments, searchAccounts, searchGroups, setAssignment, setTestUser } from '@/pages/admin/api'
import type { Environment, EnvironmentConfig, Reader } from '../api/type'
import { validateEnvironmentConfig } from '../api/validation'
import FailureFeedback from '@/components/Feedback/failure-feedback.vue'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import EntityPicker from '@/components/ui/entity-picker/entity-picker.vue'
import type { PickerOption } from '@/components/ui/entity-picker/types'
import { accountOption, groupOption } from '@/lib/picker-options'
import ConfigEditor from '../components/config-editor.vue'
import { emitToast } from '@/utils/toast'

const context = usePageContext(),
    user = useUserStore()
const provider = useAsyncQuery({
    key: 'provider-metadata',
    active: context.active,
    loader: getProviderMetadata,
})
const administrator = computed(() => context.route.value.path.startsWith('/admin/')),
    id = computed(() => Number(context.route.value.params.id))
const state = useAsyncQuery({
        key: () => `${administrator.value}:${id.value}`,
        active: context.active,
        loader: (signal) => getApp(administrator.value, id.value, signal),
    }),
    { data, status, error, refreshing } = state
const preserveEditor = ref(false)
const selected = ref(''),
    config = ref<EnvironmentConfig>(),
    metadata = reactive({ name: '', description: '' }),
    candidate = ref(0),
    candidateLabel = ref(''),
    candidateSecondary = ref(''),
    reason = ref(''),
    limitation = ref(false)
const env = computed(() =>
    data.value?.environments.find((value) => value.environment_id === selected.value),
)
const isOwner = computed(
    () => administrator.value || data.value?.owner_user_id === user.centerSession?.user_id,
)
watch(data, (app) => {
    if (preserveEditor.value) return
    metadata.name = app?.name ?? ''
    metadata.description = app?.description ?? ''
    if (app && !app.environments.some((value) => value.environment_id === selected.value))
        selected.value = app.environments.find(value => value.kind === context.route.value.query.environment)?.environment_id ?? app.environments[0]?.environment_id ?? ''
})
watch(() => context.route.value.query.environment, kind => {
    const target = data.value?.environments.find(value => value.kind === kind)
    if (target) selected.value = target.environment_id
})
watch(
    env,
    (value) => {
        if (preserveEditor.value) return
        config.value = value ? structuredClone(value.latest.config) : undefined
    },
    { immediate: true },
)
const readers = useCursorPage(
    () => `readers:${id.value}:${selected.value}`,
    (after, signal) => getReaders(administrator.value, id.value, selected.value, signal, after),
    () => context.active.value && !!selected.value,
)
const deliveries = useCursorPage(
    () => `deliveries:${id.value}:${selected.value}`,
    (after, signal) => getAppDeliveries(administrator.value, id.value, selected.value, signal, after),
    () => context.active.value && !!selected.value,
)
const loadCandidateOptions = async (
    query: string,
    signal: AbortSignal,
): Promise<PickerOption[]> => {
    if (administrator.value) {
        const page = await searchAccounts(query, signal)
        return page.items.map(accountOption)
    }
    const page = await searchCandidates(query, signal)
    return page.items.map((person) => ({
        value: person.user_id,
        label: person.nickname || person.username,
        secondary: `@${person.username} · #${person.user_id}`,
    }))
}
const deliveryStatusNames: Record<string, string> = {
    pending: '等待发送',
    leased: '发送处理中',
    retry: '等待重试',
    delivered: '已送达',
    permanent_failure: '发送失败',
    expired: '已超过期限',
}
const assignments = useAsyncQuery({
    key: () => `assignments:${id.value}`,
    active: () => context.active.value && administrator.value,
    loader: (signal) => getAssignments(id.value, signal),
})
const approval = useAsyncQuery({
    key: () => `approval:${env.value?.pending_approval ?? ''}`,
    active: () => context.active.value && !!env.value?.pending_approval,
    loader: (signal) =>
        getApproval(administrator.value, id.value, env.value!.pending_approval!, signal),
})
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation,
    operationKey = useOperationKey(),
    secret = useEphemeralResult<{ label: string; value: string }>(),
    shownSecret = secret.value
const assignment = reactive({
        kind: 'user' as 'user' | 'group',
        target: '' as string | number,
        targetLabel: '',
        targetSecondary: '',
        present: true,
    }),
    testUser = ref<number | ''>(''),
    testUserLabel = ref(''),
    testUserSecondary = ref('')
const ownerMember = computed(() =>
    data.value?.members.find((member) => member.user_id === data.value?.owner_user_id),
)
const ownerName = computed(
    () =>
        ownerMember.value?.nickname ||
        ownerMember.value?.username ||
        `账户 ${data.value?.owner_user_id ?? ''}`,
)
function memberName(member: { user_id: number; username?: string; nickname?: string }) {
    return member.nickname || member.username || `账户 ${member.user_id}`
}
function chooseCandidate(option: PickerOption) {
    candidate.value = Number(option.value)
    candidateLabel.value = option.label
    candidateSecondary.value = option.secondary ?? ''
}
function clearCandidate() {
    candidate.value = 0
    candidateLabel.value = ''
    candidateSecondary.value = ''
}
const loadAssignmentOptions = async (
    query: string,
    signal: AbortSignal,
): Promise<PickerOption[]> => {
    if (assignment.kind === 'group') {
        const page = await searchGroups(query, signal)
        return page.items.map(groupOption)
    }
    const page = await searchAccounts(query, signal)
    return page.items.map(accountOption)
}
const loadAccountOptionsForTest = async (
    query: string,
    signal: AbortSignal,
): Promise<PickerOption[]> => {
    const page = await searchAccounts(query, signal)
    return page.items.map(accountOption)
}
function chooseAssignmentTarget(option: PickerOption) {
    assignment.target = assignment.kind === 'group' ? String(option.value) : Number(option.value)
    assignment.targetLabel = option.label
    assignment.targetSecondary = option.secondary ?? ''
}
function clearAssignmentTarget() {
    assignment.target = ''
    assignment.targetLabel = ''
    assignment.targetSecondary = ''
}
function chooseTestUser(option: PickerOption) {
    testUser.value = Number(option.value)
    testUserLabel.value = option.label
    testUserSecondary.value = option.secondary ?? ''
}
function clearTestUser() {
    testUser.value = ''
    testUserLabel.value = ''
    testUserSecondary.value = ''
}
watch(() => assignment.kind, clearAssignmentTarget)
const fieldLabels: Record<keyof EnvironmentConfig, string> = {
    launch_url: '应用启动地址',
    launch_description: '启动说明',
    client_kind: '客户端类型',
    redirect_uris: '登录回调',
    post_logout_redirect_uris: '退出回调',
    browser_origins: '浏览器来源',
    access_audience: '访问 Audience',
    identity_scopes: '资料范围',
    consent_policy: '同意策略',
    logout_capability: '退出能力',
    backchannel_logout_uri: '退出通知地址',
}
type ConfigField = keyof EnvironmentConfig
interface ConfigChange {
    key: ConfigField
    label: string
    before: string
    after: string
}
function diffAgainst(
    before: EnvironmentConfig | null | undefined,
    after: EnvironmentConfig | null | undefined,
): ConfigChange[] {
    if (!after) return []
    return (Object.keys(fieldLabels) as ConfigField[])
        .filter((key) => JSON.stringify(before?.[key] ?? null) !== JSON.stringify(after[key] ?? null))
        .map((key) => ({
            key,
            label: fieldLabels[key],
            before: JSON.stringify(before?.[key] ?? null),
            after: JSON.stringify(after[key] ?? null),
        }))
}
const changes = computed(() =>
    env.value && config.value ? diffAgainst(env.value.active_config, config.value) : [],
)
const unsavedConfig = computed(() => !!env.value && !!config.value && diffAgainst(env.value.latest.config, config.value).length > 0)
const submissionIssue = computed(() => {
    if (unsavedConfig.value) return '有未保存的修改，请先保存配置提案，再提交已保存的版本。'
    return env.value ? validateEnvironmentConfig(env.value.latest.config, env.value.kind, true) : '请选择环境'
})
const approvalChanges = computed(() => {
    const fixed = approval.data.value?.config
    if (!fixed) return []
    const active = env.value?.active_config
    return (Object.keys(fieldLabels) as (keyof EnvironmentConfig)[])
        .filter(
            (key) => JSON.stringify(active?.[key] ?? null) !== JSON.stringify(fixed[key] ?? null),
        )
        .map((key) => ({
            key,
            label: fieldLabels[key],
            before: JSON.stringify(active?.[key] ?? null),
            after: JSON.stringify(fixed[key] ?? null),
        }))
})
async function recoverFailure() {
    preserveEditor.value = true
    try {
        await state.refresh()
        if (!state.error.value && !mutation.failure.value?.outcomeUncertain) mutation.clearError()
    } finally { preserveEditor.value = false }
}
async function update(path: string, method: 'POST' | 'PUT' | 'PATCH', body: unknown) {
    return mutation.run(
        () => changeApp(administrator.value, id.value, path, method, body, operationKey({ id: id.value, path, body })),
        async (receipt) => {
            if (receipt.client_secret)
                shownSecret.value = {
                    label: '客户端密钥（仅此次展示）',
                    value: receipt.client_secret,
                }
            await state.refresh()
            readers.invalidate()
            deliveries.invalidate()
            assignments.invalidate()
            approval.invalidate()
        },
    )
}
const envPath = (environment: Environment) => `/environments/${environment.environment_id}`
async function saveConfig() {
    if (!env.value || !config.value) return
    const problem = validateEnvironmentConfig(
        config.value,
        env.value.kind,
        env.value.kind === 'development',
    )
    if (problem) {
        writeError.value = problem
        return
    }
    const saved = await update(`${envPath(env.value)}/config`, 'PUT', {
        expected_revision: env.value.revision,
        config: config.value,
    })
    if (saved) emitToast({ type: 'success', message: '配置提案已保存' })
}
async function submit() {
    if (submissionIssue.value) { writeError.value = submissionIssue.value; return }
    if (env.value)
        await update(`${envPath(env.value)}/submit`, 'POST', {
            expected_revision: env.value.revision,
            version_id: env.value.latest.version_id,
        })
}
async function activate() {
    if (submissionIssue.value) { writeError.value = submissionIssue.value; return }
    if (env.value)
        await update(`${envPath(env.value)}/activate`, 'POST', {
            expected_revision: env.value.revision,
            version_id: env.value.latest.version_id,
            accept_logout_limitation: limitation.value,
        })
}
async function review(approve: boolean) {
    if (env.value?.pending_approval)
        await update(`/approvals/${env.value.pending_approval}/review`, 'POST', {
            expected_revision: env.value.revision,
            approve,
            reason: reason.value.trim() || null,
            accept_logout_limitation: limitation.value,
        })
}
async function readerChange(reader: Reader | null, reset: boolean) {
    if (!env.value?.active_config) return
    const body = reader
        ? {
              expected_revision: reader.revision,
              enabled: reset ? reader.enabled : !reader.enabled,
              reset_secret: reset,
          }
        : { audience: env.value.active_config.access_audience }
    await mutation.run(
        () =>
            changeReader(
                administrator.value,
                id.value,
                selected.value,
                reader?.reader_id ?? null,
                body,
                operationKey({ reader: reader?.reader_id, body }),
            ),
        async (receipt) => {
            if (receipt.secret)
                shownSecret.value = {
                    label: `后端内省凭据 ${receipt.reader_id}`,
                    value: receipt.secret,
                }
            await readers.refresh()
        },
    )
}
async function assign() {
    if (!data.value || assignment.target === '') return
    const target = {
        kind: assignment.kind,
        id: assignment.kind === 'user' ? Number(assignment.target) : String(assignment.target),
    }
    const body = { target, present: assignment.present, revision: data.value.revision }
    await mutation.run(
        () => setAssignment(id.value, target, assignment.present, data.value!.revision, operationKey(body)),
        async () => {
            await state.refresh()
            await assignments.refresh()
            clearAssignmentTarget()
        },
    )
}
async function test(present: boolean) {
    if (!env.value || !data.value || !testUser.value) return
    const body = {
        id: id.value,
        env: selected.value,
        user: Number(testUser.value),
        present,
        revision: data.value.revision,
    }
    await mutation.run(
        () => setTestUser(id.value, selected.value, Number(testUser.value), present, data.value!.revision, operationKey(body)),
        () => state.refresh(),
    )
}
async function toggleLifecycle() {
    if (!data.value) return
    if (data.value.active) {
        const body = { active: false, expected_revision: data.value.revision }
        const appId = id.value
        const confirmed = await mutation.confirm({
            title: '停用应用',
            content: `停用“${data.value.name}”后会撤销其全部应用授权。你可以稍后恢复应用，但用户需要重新登录。`,
            confirmText: '停用并撤销授权',
            destructive: true,
        }, () => changeApp(administrator.value, appId, '/lifecycle', 'PATCH', body, operationKey({ id: appId, path: '/lifecycle', body })))
        if (confirmed) await state.refresh()
        return
    }
    await update('/lifecycle', 'PATCH', { active: true, expected_revision: data.value.revision })
}

async function copy(value: string, label: string) {
    try {
        await navigator.clipboard.writeText(value)
        emitToast({ type: 'success', message: label + '已复制' })
    } catch {
        emitToast({ type: 'error', message: '无法访问剪贴板，请手动选择复制' })
    }
}
const envStatus = computed(() => {
    if (!env.value) return '未选择环境'
    if (env.value.pending_approval) return '生产申请待审核'
    return env.value.active_version_id ? '存在生效配置' : '尚未激活'
})
</script>

<template>
    <div class="workspace-page">
        <PageHeader
            :title="data?.name || '应用接入'"
            eyebrow="APPLICATION"
            description="开发和生产环境使用独立客户端。资料同意、生产访问分配与维护权分别管理。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="state.refresh()">
                <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />重新读取
            </UiButton>
        </PageHeader>

        <FailureFeedback :failure="mutation.failure.value" :message="writeError"
            :recovery-label="['reload', 'check-result'].includes(mutation.failure.value?.recovery ?? '') ? '加载最新数据（保留输入）' : undefined"
            :recover="recoverFailure" />

        <section v-if="shownSecret" class="panel mb-5 border-warning/30">
            <header class="panel-heading bg-warning/[0.04]">
                <div>
                    <h2>{{ shownSecret.label }}</h2>
                    <p>仅此次展示，离开页面后无法再次查看。</p>
                </div>
                <span class="status-badge status-warning">敏感信息</span>
            </header>
            <div class="surface-body space-y-3">
                <div class="copy-field">
                    <code>{{ shownSecret.value }}</code>
                    <button type="button" class="icon-button" aria-label="复制密钥" title="复制密钥" @click="copy(shownSecret.value, '密钥')">
                        <Copy :size="14" aria-hidden="true" />
                    </button>
                </div>
                <p class="form-help">存入服务端秘密配置，离开页面后清除。</p>
                <UiButton variant="outline" @click="secret.clear()">隐藏密钥</UiButton>
            </div>
        </section>

        <DataState :status="status" :error="error" @retry="state.refresh()">
            <div v-if="data" class="space-y-6">
                <nav class="section-nav" aria-label="页面章节">
                    <a href="#overview">应用概览</a>
                    <a href="#environment">环境配置</a>
                    <a href="#integration">标准接入</a>
                    <a href="#deliveries">通知与分配</a>
                </nav>

                <section id="overview" class="scroll-mt-24">
                    <div class="content-grid content-grid-sidebar">
                        <section class="panel">
                            <header class="panel-heading">
                                <div><h2>应用资料与负责人</h2><p>名称与说明会展示给授权用户和管理员。</p></div>
                            </header>
                            <form class="surface-body space-y-5" @submit.prevent="update('', 'PATCH', { name: metadata.name, description: metadata.description || null, expected_revision: data.revision })">
                                <div class="grid gap-4 md:grid-cols-2">
                                    <UiFormField id="detail-name" label="名称" required>
                                        <UiInput id="detail-name" v-model="metadata.name" required :disabled="pending" />
                                    </UiFormField>
                                    <UiFormField id="detail-description" label="说明" optional>
                                        <UiInput id="detail-description" v-model="metadata.description" :disabled="pending" placeholder="向用户说明应用用途…" />
                                    </UiFormField>
                                </div>
                                <div class="flex justify-end">
                                    <UiButton type="submit" :disabled="pending" :loading="pending">保存资料</UiButton>
                                </div>
                            </form>
                            <div class="surface-body space-y-4 border-t border-border">
                                <div class="flex flex-wrap items-center justify-between gap-3">
                                    <div>
                                        <p class="text-xs font-semibold">负责人 {{ ownerName }}</p>
                                        <p class="mt-1 text-[10.5px] text-muted-foreground">负责人账户 {{ data.owner_user_id }}</p>
                                        <p class="mt-1 text-[11px] text-muted-foreground">{{ data.management_mode === 'admin_configured' ? '管理员直接配置' : '开发者维护' }}</p>
                                    </div>
                                    <span class="status-badge" :class="data.active ? 'status-success' : 'status-muted'">{{ data.active ? '应用启用中' : '应用已停用' }}</span>
                                </div>
                                <div v-if="isOwner" class="flex flex-wrap items-end gap-3 rounded-xl border border-border bg-muted/25 p-3">
                                    <UiFormField id="detail-candidate" label="交接或添加维护者" class="min-w-[220px] flex-1" hint="按显示名称、用户名或账户 ID 搜索。">
                                        <EntityPicker
                                            id="detail-candidate"
                                            :model-value="candidate || null"
                                            :selected-label="candidateLabel"
                                            :selected-secondary="candidateSecondary"
                                            :load="loadCandidateOptions"
                                            placeholder="搜索账户名称或用户名…"
                                            :disabled="pending"
                                            @update:model-value="(value) => { if (value === null) clearCandidate() }"
                                            @select="chooseCandidate"
                                        />
                                    </UiFormField>
                                    <UiButton variant="outline" :disabled="pending || !candidate" @click="update('/owner', 'PATCH', { user_id: Number(candidate), expected_revision: data.revision })">交接负责人</UiButton>
                                    <UiButton v-if="!administrator" variant="outline" :disabled="pending || !candidate" @click="update(`/members/${Number(candidate)}`, 'PATCH', { present: true, expected_revision: data.revision })">添加维护者</UiButton>
                                </div>
                                <div class="space-y-2">
                                    <div v-for="member in data.members" :key="member.user_id" class="flex flex-wrap items-center gap-2 rounded-lg border border-border px-3 py-2 text-xs">
                                        <span class="flex h-7 w-7 items-center justify-center rounded-lg bg-secondary text-[10px] font-semibold">{{ memberName(member).slice(0, 1).toUpperCase() }}</span>
                                        <span class="min-w-0 flex-1">
                                            <span class="block font-medium">{{ memberName(member) }}</span>
                                            <span class="mt-0.5 block text-[10.5px] text-muted-foreground">账户 {{ member.user_id }} · {{ member.member_role === 'owner' ? '负责人' : '维护者' }}<template v-if="member.username"> · @{{ member.username }}</template></span>
                                        </span>
                                        <UiButton v-if="!administrator && isOwner && member.member_role !== 'owner'" variant="ghost" size="sm" class="ml-auto" :disabled="pending" @click="update(`/members/${member.user_id}`, 'PATCH', { present: false, expected_revision: data.revision })">移除维护者</UiButton>
                                    </div>
                                </div>
                                <UiButton v-if="administrator" variant="outline" class="!border-destructive/25 !text-destructive hover:!bg-destructive/5" :disabled="pending" :loading="pending" @click="toggleLifecycle">{{ data.active ? '停用应用并撤销授权' : '恢复应用（重检负责人）' }}</UiButton>
                            </div>
                        </section>

                        <aside class="section-stack sticky-column">
                            <section class="surface p-5">
                                <h2 class="flex items-center gap-2 text-xs font-semibold"><ShieldCheck :size="14" class="text-primary" aria-hidden="true" />接入概览</h2>
                                <dl class="mt-4 space-y-3 text-[11.5px]">
                                    <div class="flex justify-between gap-3"><dt class="text-muted-foreground" title="中心内部稳定标识；登录接入使用对应环境的 Client ID。">应用标识</dt><dd class="mono break-all text-right">{{ data.app_key }}</dd></div>
                                    <div class="flex justify-between gap-3"><dt class="text-muted-foreground">管理方式</dt><dd class="text-right">{{ data.management_mode === 'admin_configured' ? '管理员直接配置' : '开发者维护' }}</dd></div>
                                    <div class="flex justify-between gap-3"><dt class="text-muted-foreground">应用归属</dt><dd class="text-right">{{ data.third_party ? '第三方应用' : '内部应用' }}</dd></div>
                                </dl>
                            </section>
                            <section class="surface p-5">
                                <h2 class="flex items-center gap-2 text-xs font-semibold"><Users :size="14" class="text-primary" aria-hidden="true" />维护成员</h2>
                                <p class="mt-2 text-[11px] leading-5 text-muted-foreground">负责人可交接维护权；维护者只能管理此应用，不自动获得生产访问。</p>
                                <p class="mt-3 text-[11px]">{{ data.members.length }} 位成员</p>
                            </section>
                        </aside>
                    </div>
                </section>

                <section id="environment" class="panel scroll-mt-24">
                    <header class="panel-heading">
                        <div><h2>应用环境</h2><p>开发和生产的客户端、回调与访问边界独立维护。</p></div>
                        <span class="status-badge" :class="env?.pending_approval ? 'status-warning' : env?.active_version_id ? 'status-success' : 'status-muted'">{{ envStatus }}</span>
                    </header>
                    <div class="surface-body space-y-5">
                        <div class="flex flex-wrap items-end gap-3">
                            <UiFormField id="detail-environment" label="当前环境" class="min-w-[220px]">
                                <select id="detail-environment" v-model="selected" class="ui-select">
                                    <option v-for="environment in data.environments" :key="environment.environment_id" :value="environment.environment_id">{{ environment.kind === 'development' ? '开发环境' : '生产环境' }}</option>
                                </select>
                            </UiFormField>
                            <template v-if="env">
                                <div class="mb-1 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                                    <span>{{ env.pending_approval ? '等待管理员审核' : env.active_version_id ? '存在生效配置' : '尚未激活' }}</span>
                                    <span>· 最新版本 {{ env.latest.version }}</span>
                                </div>
                            </template>
                        </div>

                        <template v-if="env">
                            <div class="grid gap-3 md:grid-cols-2">
                                <div class="copy-field">
                                    <div class="min-w-0">
                                        <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Client ID：</p>
                                        <code class="mt-1 block break-all">{{ env.client_id }}</code>
                                    </div>
                                    <button type="button" class="icon-button" aria-label="复制 Client ID" title="复制 Client ID" @click="copy(env.client_id, 'Client ID')">
                                        <Copy :size="13" aria-hidden="true" />
                                    </button>
                                </div>
                                <div class="rounded-lg border border-border bg-muted/30 p-3">
                                    <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">环境 ID</p>
                                    <p class="mono mt-1 break-all text-[11.5px]">{{ env.environment_id }}</p>
                                </div>
                            </div>
                            <p v-if="env.latest.candidate_client_id" class="form-help">类型切换候选 Client ID：<span class="mono">{{ env.latest.candidate_client_id }}</span></p>

                            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                                <form class="rounded-xl border border-border p-4" @submit.prevent="saveConfig">
                                    <h2 class="mb-1 text-xs font-semibold">配置提案</h2>
                                    <p class="mb-4 text-[11px] text-muted-foreground">修改会先保存为提案，再按环境治理规则生效。</p>
                                    <ConfigEditor v-if="config" v-model="config" prefix="detail-config" :environment="env.kind" :administrator="administrator" :disabled="pending" />
                                    <UiButton class="mt-4" type="submit" :disabled="pending" :loading="pending">{{ env.kind === 'development' ? '保存并更新开发配置' : '保存生产配置提案' }}</UiButton>
                                </form>
                                <div class="space-y-4">
                                    <section class="rounded-xl border border-border p-4">
                                        <h2 class="mb-3 text-xs font-semibold">与生效配置的差异</h2>
                                        <p v-if="!changes.length" class="text-[11px] text-muted-foreground">配置与生效版本一致。</p>
                                        <dl v-for="change in changes" :key="change.key" class="mb-3 rounded-lg bg-muted p-3 text-[11px]">
                                            <dt class="font-semibold">{{ change.label }}</dt>
                                            <dd class="mt-1 break-all">当前：{{ change.before }}</dd>
                                            <dd class="mt-1 break-all">提案：{{ change.after }}</dd>
                                        </dl>
                                        <template v-if="env.kind === 'production'">
                                            <p v-if="submissionIssue" class="mb-3 text-[11px] text-muted-foreground">{{ submissionIssue }}</p>
                                            <UiButton v-if="!administrator" :disabled="pending || !!env.pending_approval || !!submissionIssue" :loading="pending" @click="submit">提交已保存的最新版本供审核</UiButton>
                                            <template v-else>
                                                <label class="checkbox-field mt-3"><input v-model="limitation" type="checkbox" /><span>接受并记录退出能力限制</span></label>
                                            <UiButton v-if="data.management_mode === 'admin_configured'" class="mt-3" :disabled="pending || !!submissionIssue" :loading="pending" @click="activate">激活已保存的生产版本</UiButton>
                                                <template v-if="env.pending_approval">
                                                    <div class="mt-4 rounded-lg border border-warning/25 bg-warning/[0.04] p-3">
                                                        <p class="text-[11px] font-semibold">待审核申请</p>
                                                        <p class="mt-1 text-[11px] text-muted-foreground">申请人账户 {{ approval.data.value?.requested_by }} · {{ approval.data.value?.status === 'pending' ? '待审核' : approval.data.value?.status }}</p>
                                                        <div v-if="approval.data.value" class="mt-3">
                                                            <h3 class="text-xs font-semibold">固定申请快照对比</h3>
                                                            <p class="mt-1 text-[10.5px] text-muted-foreground">审核对象为提交时固定的版本 {{ approval.data.value.version_id }}，不随后续编辑变化。</p>
                                                            <p v-if="!approvalChanges.length" class="mt-2 text-[11px]">与当前生效配置一致。</p>
                                                            <dl v-for="change in approvalChanges" :key="'approval-' + change.key" class="mb-2 rounded bg-card p-2 text-[10.5px]">
                                                                <dt class="font-semibold">{{ change.label }}</dt>
                                                                <dd class="mt-1 break-all">当前：{{ change.before }}</dd>
                                                                <dd class="mt-1 break-all">申请：{{ change.after }}</dd>
                                                            </dl>
                                                            <p class="mt-2 text-[10.5px]">快照负责人账户 {{ approval.data.value.snapshot.owner_user_id }} · 退出限制已接受：{{ approval.data.value.logout_limitation_accepted ? '是' : '否' }}</p>
                                                        </div>
                                                        <UiFormField id="review-reason" label="审批原因" optional class="mt-3">
                                                            <UiInput id="review-reason" v-model="reason" :disabled="pending" placeholder="记录批准或拒绝的原因…" />
                                                        </UiFormField>
                                                        <div class="mt-3 flex flex-wrap gap-2">
                                                            <UiButton :disabled="pending || approval.data.value?.requested_by === user.centerSession?.user_id" :loading="pending" @click="review(true)">批准固定申请快照</UiButton>
                                                            <UiButton variant="outline" :disabled="pending" @click="review(false)">拒绝申请</UiButton>
                                                        </div>
                                                        <p v-if="approval.error.value" role="alert" class="inline-error mt-3">{{ approval.error.value }}</p>
                                                    </div>
                                                </template>
                                            </template>
                                        </template>
                                    </section>
                                </div>
                            </div>
                        </template>
                    </div>
                </section>

                <section id="integration" class="scroll-mt-24">
                    <div class="content-grid content-grid-2">
                        <section class="panel">
                            <header class="panel-heading"><div><h2>标准接入配置</h2><p>从 Discovery 获取协议端点。</p></div></header>
                            <div class="surface-body space-y-3 text-[11.5px] leading-6">
                                <div class="copy-field">
                                    <div class="min-w-0">
                                        <p class="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">Issuer</p>
                                        <code class="mt-1 block break-all">{{ provider.data.value?.issuer }}</code>
                                    </div>
                                    <button type="button" class="icon-button" aria-label="复制 Issuer" title="复制 Issuer" @click="copy(provider.data.value?.issuer ?? '', 'Issuer')"><Copy :size="13" aria-hidden="true" /></button>
                                </div>
                                <p class="break-all text-muted-foreground">Discovery：{{ provider.data.value?.issuer }}/.well-known/openid-configuration</p>
                                <p class="text-muted-foreground">授权码流程要求 S256 PKCE、state 与 nonce；验证签名、issuer、audience。保护 API 在线内省缓存最长 5 秒。</p>
                                <p v-if="env?.active_config">生效范围 {{ env.active_config.identity_scopes }} · Audience <span class="mono">{{ env.active_config.access_audience }}</span></p>
                                <p v-else class="text-muted-foreground">此环境尚无生效接入配置。</p>
                                <UiButton
                                    v-if="env && env.primary_client_kind === 'server_web'"
                                    variant="outline"
                                    :disabled="pending"
                                    :loading="pending"
                                    @click="update(`${envPath(env)}/secret/reset`, 'POST', { expected_revision: env.revision })"
                                    >重置本环境客户端密钥</UiButton
                                >
                            </div>
                        </section>

                        <section class="panel">
                            <header class="panel-heading"><div><h2>保护 API 内省凭据</h2><p>仅交给此环境的后端 API；浏览器不能保存该秘密。</p></div></header>
                            <div class="surface-body space-y-3">
                                <UiButton :disabled="pending || !env?.active_config" :loading="pending" variant="outline" @click="readerChange(null, false)"><KeyRound :size="14" aria-hidden="true" />为生效 Audience 创建后端凭据</UiButton>
                                <p v-if="readers.error.value" role="alert" class="inline-error">{{ readers.error.value }}</p>
                                <div v-if="readers.data.value?.items.length" class="space-y-2">
                                    <article v-for="reader in readers.data.value?.items" :key="reader.id" class="rounded-lg border border-border p-3">
                                        <p class="mono break-all text-[11px]">{{ reader.reader_id }} · {{ reader.enabled ? '启用' : '停用' }} · {{ reader.approved_audience }}</p>
                                        <div class="mt-2 flex gap-2">
                                            <UiButton variant="ghost" size="sm" :disabled="pending" @click="readerChange(reader, true)">重置秘密</UiButton>
                                            <UiButton variant="ghost" size="sm" :disabled="pending" @click="readerChange(reader, false)">{{ reader.enabled ? '停用' : '启用' }}</UiButton>
                                        </div>
                                    </article>
                                </div>
                                <p v-else class="text-[11px] text-muted-foreground">此环境尚无后端内省凭据。</p>
                                <UiButton v-if="readers.data.value?.next" variant="outline" size="sm" :disabled="pending" @click="readers.next()">下一页凭据</UiButton>
                            </div>
                        </section>
                    </div>
                </section>

                <section id="deliveries" class="scroll-mt-24">
                    <div class="content-grid" :class="administrator ? 'content-grid-2' : ''">
                        <section class="panel">
                            <header class="panel-heading"><div><h2>退出通知投递</h2><p>对端返回 2xx 只表示接受通知；本地授权撤销已在此之前生效。</p></div></header>
                            <div class="surface-body space-y-2">
                                <p v-if="deliveries.error.value" class="inline-error">{{ deliveries.error.value }}</p>
                                <article v-for="delivery in deliveries.data.value?.items" :key="delivery.id" class="rounded-lg border border-border p-3">
                                    <p class="text-xs font-medium">{{ deliveryStatusNames[delivery.status] || delivery.status }} · 尝试 {{ delivery.attempt_count }} 次{{ delivery.last_error_code ? ' · ' + delivery.last_error_code : '' }}</p>
                                    <p class="mt-1 text-[10.5px] text-muted-foreground">建立 {{ new Date(delivery.created_at * 1000).toLocaleString('zh-CN') }}{{ delivery.delivered_at ? ' · 送达 ' + new Date(delivery.delivered_at * 1000).toLocaleString('zh-CN') : '' }}</p>
                                </article>
                                <p v-if="deliveries.data.value && !deliveries.data.value.items.length" class="text-[11px] text-muted-foreground">此环境尚无退出通知投递。</p>
                                <UiButton v-if="deliveries.data.value?.next" variant="outline" size="sm" :disabled="pending" @click="deliveries.next()">下一页投递</UiButton>
                            </div>
                        </section>

                        <section v-if="administrator" class="panel">
                            <header class="panel-heading"><div><h2>生产访问分配</h2><p>只有明确分配的账户或用户组才能访问此环境。</p></div></header>
                            <div class="surface-body space-y-4">
                                <p v-if="assignments.error.value" class="inline-error">{{ assignments.error.value }}</p>
                                <div class="flex flex-wrap gap-1.5">
                                    <span v-for="entry in assignments.data.value" :key="entry.target.kind + ':' + entry.target.id" class="flex max-w-full flex-col rounded-lg border border-border bg-muted/30 px-2.5 py-1.5">
                                        <span class="truncate text-[11px] font-semibold">{{ entry.label }}</span>
                                        <span class="truncate text-[9.5px] text-muted-foreground">{{ entry.secondary }}</span>
                                    </span>
                                    <span v-if="!assignments.data.value?.length" class="text-[11px] text-muted-foreground">尚未明确分配</span>
                                </div>
                                <form class="flex flex-wrap items-end gap-3" @submit.prevent="assign">
                                    <UiFormField id="assign-kind" label="对象类型" class="w-32">
                                        <select id="assign-kind" v-model="assignment.kind" class="ui-select"><option value="user">账户</option><option value="group">用户组</option></select>
                                    </UiFormField>
                                    <UiFormField id="assign-id" label="搜索并选择对象" class="min-w-[220px] flex-1">
                                        <EntityPicker
                                            id="assign-id"
                                            :model-value="assignment.target"
                                            :selected-label="assignment.targetLabel"
                                            :selected-secondary="assignment.targetSecondary"
                                            :load="loadAssignmentOptions"
                                            :placeholder="assignment.kind === 'group' ? '搜索用户组名称…' : '搜索账户名称或用户名…'"
                                            :disabled="pending"
                                            @update:model-value="(value) => { if (value === null) clearAssignmentTarget() }"
                                            @select="chooseAssignmentTarget"
                                        />
                                    </UiFormField>
                                    <label class="checkbox-field mb-2"><input v-model="assignment.present" type="checkbox" /><span>授予访问（取消为移除）</span></label>
                                    <UiButton type="submit" :disabled="pending" :loading="pending">更新明确分配</UiButton>
                                </form>
                                <div v-if="env?.kind === 'development'" class="flex flex-wrap items-end gap-3 border-t border-border pt-4">
                                    <UiFormField id="test-user" label="开发测试账户" class="min-w-[220px] flex-1">
                                        <EntityPicker
                                            id="test-user"
                                            :model-value="testUser"
                                            :selected-label="testUserLabel"
                                            :selected-secondary="testUserSecondary"
                                            :load="loadAccountOptionsForTest"
                                            placeholder="搜索测试账户名称或用户名…"
                                            :disabled="pending"
                                            @update:model-value="(value) => { if (value === null) clearTestUser() }"
                                            @select="chooseTestUser"
                                        />
                                    </UiFormField>
                                    <UiButton :disabled="pending || !testUser" :loading="pending" @click="test(true)">添加测试访问</UiButton>
                                    <UiButton variant="outline" :disabled="pending || !testUser" @click="test(false)">移除测试访问</UiButton>
                                </div>
                            </div>
                        </section>
                    </div>
                </section>
            </div>
        </DataState>
    </div>
</template>
