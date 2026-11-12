<script setup lang="ts">
defineOptions({ name: 'DeveloperApps' })
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { ArrowRight, Copy, Plus, Search, ShieldCheck, Sparkles, X } from '@lucide/vue'
import { usePageContext } from '@/workspace/page-context'
import { searchAccounts } from '@/pages/admin/api'
import { useUserStore } from '@/pinia/stores/user'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult, useOperationKey } from '@/composables/use-ephemeral-result'
import { useFormDraft } from '@/composables/use-form-draft'
import { listApps, registerApp, registerAdminApp } from '../api'
import { emptyConfig, type Registration, type RegistrationReceipt } from '../api/type'
import {
    environmentConfigIssue,
    registrationBasicIssue,
    registrationIssue,
    type EnvironmentConfigField,
    type ValidationIssue,
} from '../api/validation'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import ConfigEditor from '../components/config-editor.vue'
import EntityPicker from '@/components/ui/entity-picker/entity-picker.vue'
import type { PickerOption } from '@/components/ui/entity-picker/types'
import { accountOption } from '@/lib/picker-options'
import { emitToast } from '@/utils/toast'

type StepId = 'basic' | 'development' | 'production' | 'confirm'

const context = usePageContext()
const user = useUserStore()
const administrator = computed(() => context.route.value.path.startsWith('/admin/'))
const list = useCursorPage('managed-apps', (after: number | undefined, signal) =>
    listApps(administrator.value, after, signal),
)
const { data, status, error, refreshing } = list

const registrationSteps: readonly { id: StepId; title: string; description: string }[] = [
    { id: 'basic', title: '基础信息', description: '填写应用身份与负责人，后续配置会沿用这些信息。' },
    { id: 'development', title: '开发环境', description: '配置本机或测试环境回调。开发环境建立后立即生效。' },
    { id: 'production', title: '生产环境', description: '生产配置需独立审批；还未准备好时可先留空。' },
    { id: 'confirm', title: '确认建立', description: '核对两环境配置。提交后会创建应用与环境。' },
]
const opened = ref(false)
const formRef = ref<HTMLFormElement | null>(null)
const owner = ref(user.centerSession?.user_id ?? 0),
    ownerLabel = ref(user.centerSession?.nickname || user.centerSession?.username || ''),
    ownerSecondary = ref(
        user.centerSession
            ? `@${user.centerSession.username} · #${user.centerSession.user_id}`
            : '',
    )
const loadOwnerOptions = async (
    query: string,
    signal: AbortSignal,
): Promise<PickerOption[]> => {
    const page = await searchAccounts(query, signal)
    return page.items.map(accountOption)
}
function chooseOwner(option: PickerOption) {
    owner.value = Number(option.value)
    ownerLabel.value = option.label
    ownerSecondary.value = option.secondary ?? ''
}
const limitation = ref(false)
const step = ref<StepId>('basic')
const unlockedStep = ref(0)
const query = ref('')
const stepIndex = computed(() => registrationSteps.findIndex((item) => item.id === step.value))
const currentStep = computed(() => registrationSteps[stepIndex.value]!)
const fieldErrors = reactive<Record<string, string>>({})
const form = reactive<Registration>({
    app_key: '',
    name: '',
    description: '',
    third_party: false,
    development: emptyConfig(),
    production: emptyConfig(),
})
const result = useEphemeralResult<RegistrationReceipt>()
const receipt = result.value
const mutation = useAsyncMutation()
const { pending, error: writeError } = mutation
const operationKey = useOperationKey()

const draft = useFormDraft('应用注册', () => ({ form, owner: owner.value, limitation: limitation.value }), {
    enabled: opened,
    submitting: pending,
    discard: resetRegistration,
})

watch(
    [form, owner, limitation],
    () => {
        clearFieldErrors()
    },
    { deep: true },
)

const filteredApps = computed(() => {
    const keyword = query.value.trim().toLowerCase()
    const items = data.value?.items ?? []
    if (!keyword) return items
    return items.filter(
        (app) =>
            app.name.toLowerCase().includes(keyword) ||
            app.app_key.toLowerCase().includes(keyword) ||
            (app.description ?? '').toLowerCase().includes(keyword),
    )
})
const pendingApprovals = computed(
    () =>
        data.value?.items.reduce(
            (total, app) =>
                total + app.environments.filter((environment) => environment.pending_approval).length,
            0,
        ) ?? 0,
)

function clearFieldErrors() {
    for (const key of Object.keys(fieldErrors)) delete fieldErrors[key]
}

function resetRegistration() {
    Object.assign(form, {
        app_key: '',
        name: '',
        description: '',
        third_party: false,
        development: emptyConfig(),
        production: emptyConfig(),
    })
    owner.value = user.centerSession?.user_id ?? 0
    ownerLabel.value = user.centerSession?.nickname || user.centerSession?.username || ''
    ownerSecondary.value = user.centerSession
        ? `@${user.centerSession.username} · #${user.centerSession.user_id}`
        : ''
    limitation.value = false
    step.value = 'basic'
    unlockedStep.value = 0
    opened.value = false
    clearFieldErrors()
}

async function toggleRegistration() {
    if (opened.value) {
        if (!(await draft.confirmDiscard())) return
        resetRegistration()
        draft.markSaved()
        return
    }
    resetRegistration()
    draft.markSaved()
    opened.value = true
    await nextTick()
    formRef.value?.querySelector<HTMLElement>('#app-key')?.focus()
}

function ownerIssue(): ValidationIssue | null {
    if (!administrator.value) return null
    if (!Number.isInteger(owner.value) || owner.value < 1)
        return { field: 'owner_user_id', message: '请选择负责人账户' }
    return null
}

function limitationIssue(): ValidationIssue | null {
    if (!administrator.value || limitation.value) return null
    return {
        field: 'accept_logout_limitation',
        message: '需要接受已记录的退出能力限制后才能建立应用',
    }
}

function currentIssue(): ValidationIssue | null {
    if (step.value === 'basic')
        return ownerIssue() ?? registrationBasicIssue(form, administrator.value)
    if (step.value === 'development')
        return environmentConfigIssue(form.development, 'development', true)
    if (step.value === 'production')
        return environmentConfigIssue(form.production, 'production', administrator.value)
    return limitationIssue() ?? registrationIssue(form, administrator.value)
}

function setIssue(issue: ValidationIssue) {
    fieldErrors[issue.field] = issue.message
}

const issueControls: Record<string, string> = {
    app_key: 'app-key',
    name: 'app-name',
    description: 'app-description',
    owner_user_id: 'app-owner',
    accept_logout_limitation: 'accept-limitation',
    client_kind: 'kind',
    redirect_uris: 'redirects',
    post_logout_redirect_uris: 'logout-redirects',
    browser_origins: 'origins',
    access_audience: 'audience',
    identity_scopes: 'scopes',
    consent_policy: 'consent',
    logout_capability: 'logout',
    backchannel_logout_uri: 'backchannel',
}

function focusIssue(issue: ValidationIssue) {
    let field = issue.field
    let environment = ''
    if (field.startsWith('development.') || field.startsWith('production.')) {
        environment = field.slice(0, field.indexOf('.'))
        field = field.slice(field.indexOf('.') + 1)
    }
    const control = issueControls[field]
    const id = control
        ? environment
            ? `registration-${environment === 'development' ? 'dev' : 'prod'}-${control}`
            : control
        : ''
    const element = id ? formRef.value?.querySelector<HTMLElement>(`#${id}`) ?? null : null
    if (element) {
        element.focus()
        return
    }
    formRef.value?.querySelector<HTMLElement>('#registration-step-title')?.focus()
}

async function focusStepTitle() {
    await nextTick()
    formRef.value?.querySelector<HTMLElement>('#registration-step-title')?.focus()
}

async function goToStep(index: number) {
    if (pending.value || index === stepIndex.value || index < 0 || index >= registrationSteps.length)
        return
    if (index > stepIndex.value + 1) return
    if (index > unlockedStep.value && index !== stepIndex.value + 1) return
    if (index === stepIndex.value + 1) {
        const issue = currentIssue()
        if (issue) {
            const scopedIssue =
                step.value === 'development' || step.value === 'production'
                    ? { ...issue, field: `${step.value}.${issue.field}` }
                    : issue
            setIssue(scopedIssue)
            await nextTick()
            focusIssue(scopedIssue)
            return
        }
        unlockedStep.value = index
    }
    step.value = registrationSteps[index]!.id
    await focusStepTitle()
}

function nextStep() {
    return goToStep(stepIndex.value + 1)
}

function previousStep() {
    return goToStep(stepIndex.value - 1)
}

function fieldError(field: string) {
    return fieldErrors[field]
}

function environmentErrors(environment: 'development' | 'production') {
    const errors: Partial<Record<EnvironmentConfigField, string>> = {}
    const prefix = `${environment}.`
    for (const [field, message] of Object.entries(fieldErrors)) {
        if (field.startsWith(prefix))
            errors[field.slice(prefix.length) as EnvironmentConfigField] = message
    }
    return errors
}

const developmentErrors = computed(() => environmentErrors('development'))
const productionErrors = computed(() => environmentErrors('production'))
const clientKindLabel = (kind: Registration['development']['client_kind']) =>
    kind === 'browser_spa' ? '浏览器 SPA' : '服务端 Web'
const uriList = (values: string[], emptyText: string) =>
    values.length ? values.join('；') : emptyText

async function register() {
    const metaIssue = ownerIssue() ?? limitationIssue()
    const issue = metaIssue ?? registrationIssue(form, administrator.value)
    if (issue) {
        setIssue(issue)
        if (issue.field.startsWith('development.')) {
            step.value = 'development'
            unlockedStep.value = Math.max(unlockedStep.value, 1)
        } else if (issue.field.startsWith('production.')) {
            step.value = 'production'
            unlockedStep.value = Math.max(unlockedStep.value, 2)
        }
        await nextTick()
        focusIssue(issue)
        return
    }
    const data: Registration = { ...form, description: form.description?.trim() || null }
    await mutation.run(
        () => {
            return administrator.value
                ? registerAdminApp(
                      {
                          application: data,
                          owner_user_id: owner.value,
                          assignments: [],
                          accept_logout_limitation: limitation.value,
                      },
                      operationKey({ data, owner: owner.value, limitation: limitation.value }),
                  )
                : registerApp(data, operationKey(data))
        },
        async (value) => {
            receipt.value = value
            resetRegistration()
            draft.markSaved()
            await list.refresh()
            emitToast({ type: 'success', message: '应用已建立，密钥仅在回执中展示一次' })
        },
    )
}

async function copy(value: string, label: string) {
    try {
        await navigator.clipboard.writeText(value)
        emitToast({ type: 'success', message: label + '已复制' })
    } catch {
        emitToast({ type: 'error', message: '无法访问剪贴板，请手动选择复制' })
    }
}
</script>

<template>
    <div class="workspace-page">
        <PageHeader
            :title="administrator ? '企业应用' : '开发者控制台'"
            eyebrow="APPLICATIONS"
            :description="administrator ? '审核生产接入、管理分配和直接配置获批第三方。' : '一次注册建立开发与生产环境，生产接入需独立审批。'"
        >
            <UiButton :aria-expanded="opened" @click="toggleRegistration">
                <Plus v-if="!opened" :size="14" aria-hidden="true" />
                <X v-else :size="14" aria-hidden="true" />
                {{ opened ? '收起注册表单' : administrator ? '配置获批第三方' : '注册应用' }}
            </UiButton>
        </PageHeader>

        <p v-if="writeError" role="alert" class="form-alert mb-4">{{ writeError }}</p>

        <div class="metric-grid mb-5">
            <article class="metric-card">
                <span class="metric-label"><Sparkles :size="13" aria-hidden="true" />应用总数</span>
                <strong class="metric-value">{{ data?.items.length ?? 0 }}</strong>
                <span class="metric-meta">当前页面加载的应用</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><ShieldCheck :size="13" aria-hidden="true" />启用中</span>
                <strong class="metric-value">{{ data?.items.filter((app) => app.active).length ?? 0 }}</strong>
                <span class="metric-meta">可以继续维护接入配置</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><ArrowRight :size="13" aria-hidden="true" />待审批环境</span>
                <strong class="metric-value">{{ pendingApprovals }}</strong>
                <span class="metric-meta">生产配置等待管理员审核</span>
            </article>
        </div>

        <section v-if="receipt" class="panel mb-5 border-success/30" aria-live="polite">
            <header class="panel-heading bg-success/[0.04]">
                <div>
                    <h2>应用已建立</h2>
                    <p>操作回执 <code class="mono break-all">{{ receipt.operation_id }}</code></p>
                </div>
                <span class="status-badge status-success">成功</span>
            </header>
            <div class="surface-body space-y-4">
                <div v-for="secret in receipt.secrets || receipt.newly_issued_secrets || []" :key="secret.environment_id">
                    <p class="mb-1.5 text-[11px] font-semibold">
                        {{ receipt.environments.find((e) => e.environment_id === secret.environment_id)?.kind === 'development' ? '开发环境' : '生产环境' }}密钥（仅此次展示）
                    </p>
                    <div class="copy-field">
                        <code>{{ secret.client_secret }}</code>
                        <button type="button" class="icon-button" aria-label="复制密钥" title="复制密钥" @click="copy(secret.client_secret, '密钥')">
                            <Copy :size="14" aria-hidden="true" />
                        </button>
                    </div>
                </div>
                <p class="form-help">请保存到应用服务端的秘密配置中。离开页面后清除；响应丢失时可在环境详情重置密钥。</p>
                <div class="flex flex-wrap gap-2">
                    <RouterLink
                        class="ui-button ui-button-default"
                        :to="{ name: administrator ? 'admin-app' : 'developer-app', params: { id: receipt.application_id } }"
                    >查看接入配置</RouterLink>
                    <UiButton variant="outline" @click="result.clear()">我已保存，隐藏回执</UiButton>
                </div>
            </div>
        </section>

        <form
            v-if="opened"
            ref="formRef"
            class="registration-panel mb-5"
            :aria-busy="pending"
            novalidate
            @submit.prevent="register"
        >
            <nav class="registration-steps" aria-label="应用注册步骤">
                <ol class="wizard-steps">
                    <li v-for="(item, index) in registrationSteps" :key="item.id">
                        <button
                            type="button"
                            class="wizard-step-button"
                            :disabled="pending || index > unlockedStep"
                            :aria-current="step === item.id ? 'step' : undefined"
                            @click="goToStep(index)"
                        >
                            <span class="wizard-step-index" aria-hidden="true">{{ index + 1 }}</span>
                            <span>{{ item.title }}</span>
                        </button>
                    </li>
                </ol>
            </nav>

            <div class="registration-body">
                <header class="registration-step-header">
                    <div class="min-w-0">
                        <h2 id="registration-step-title" class="text-base font-semibold" tabindex="-1">{{ currentStep.title }}</h2>
                        <p class="mt-1 text-sm text-muted-foreground">
                            {{ step === 'production' ? (administrator ? '生产配置会立即生效，必须完整填写。' : '生产配置需独立审批；还未准备好时可先留空。') : currentStep.description }}
                        </p>
                    </div>
                    <span class="step-counter" aria-hidden="true">{{ stepIndex + 1 }} / {{ registrationSteps.length }}</span>
                </header>

                <section v-if="step === 'basic'" class="space-y-5">
                    <div class="grid gap-4 md:grid-cols-2">
                        <UiFormField id="app-key" label="应用 Key" required :error="fieldError('app_key')" error-id="app-key-error">
                            <UiInput
                                id="app-key"
                                v-model="form.app_key"
                                :invalid="!!fieldError('app_key')"
                                name="app-key"
                                required
                                autocomplete="off"
                                spellcheck="false"
                                placeholder="yufu-spa-example…"
                                :disabled="pending"
                            />
                        </UiFormField>
                        <UiFormField id="app-name" label="应用名称" required :error="fieldError('name')" error-id="app-name-error">
                            <UiInput
                                id="app-name"
                                v-model="form.name"
                                :invalid="!!fieldError('name')"
                                name="app-name"
                                required
                                autocomplete="off"
                                placeholder="Yufu SPA 示例…"
                                :disabled="pending"
                            />
                        </UiFormField>
                        <UiFormField id="app-description" label="说明" optional class="md:col-span-2" :error="fieldError('description')" error-id="app-description-error">
                            <UiInput
                                id="app-description"
                                v-model="form.description"
                                :invalid="!!fieldError('description')"
                                name="app-description"
                                autocomplete="off"
                                placeholder="向管理员说明用途…"
                                :disabled="pending"
                            />
                        </UiFormField>
                        <UiFormField v-if="administrator" id="app-owner" label="负责人账户" class="md:col-span-2" required :error="fieldError('owner_user_id')" error-id="app-owner-error" hint="按显示名称、用户名或账户 ID 搜索。">
                            <EntityPicker
                                id="app-owner"
                                :model-value="owner"
                                :selected-label="ownerLabel"
                                :selected-secondary="ownerSecondary"
                                :load="loadOwnerOptions"
                                placeholder="搜索负责人姓名或用户名…"
                                :invalid="!!fieldError('owner_user_id')"
                                :disabled="pending"
                                @update:model-value="(value) => { if (value === null) { owner = 0; ownerLabel = ''; ownerSecondary = '' } }"
                                @select="chooseOwner"
                            />
                        </UiFormField>
                    </div>
                    <label class="checkbox-field">
                        <input v-model="form.third_party" type="checkbox" :disabled="pending" />
                        <span>第三方应用</span>
                    </label>
                    <p class="form-help">第三方应用会进入管理员配置流程；内部应用默认由开发者维护。</p>
                </section>

                <section v-else-if="step === 'development'" class="space-y-4">
                    <ConfigEditor
                        v-model="form.development"
                        environment="development"
                        prefix="registration-dev"
                        :activation="true"
                        :administrator="administrator"
                        :disabled="pending"
                        :errors="developmentErrors"
                    />
                </section>

                <section v-else-if="step === 'production'" class="space-y-4">
                    <ConfigEditor
                        v-model="form.production"
                        environment="production"
                        prefix="registration-prod"
                        :activation="administrator"
                        :administrator="administrator"
                        :disabled="pending"
                        :errors="productionErrors"
                    />
                </section>

                <section v-else class="space-y-5" aria-label="注册信息确认">
                    <div class="summary-grid">
                        <section class="summary-card">
                            <h3>基础信息</h3>
                            <dl class="summary-list">
                                <div><dt>应用 Key</dt><dd><code class="break-all" translate="no">{{ form.app_key }}</code></dd></div>
                                <div><dt>应用名称</dt><dd class="break-words">{{ form.name }}</dd></div>
                                <div><dt>说明</dt><dd class="break-words">{{ form.description || '未填写' }}</dd></div>
                                <div><dt>应用归属</dt><dd>{{ form.third_party ? '第三方应用' : '内部应用' }}</dd></div>
                            </dl>
                            <UiButton size="sm" variant="outline" :disabled="pending" @click="goToStep(0)">修改基础信息</UiButton>
                        </section>
                        <section class="summary-card">
                            <h3>开发环境</h3>
                            <dl class="summary-list">
                                <div><dt>应用类型</dt><dd>{{ clientKindLabel(form.development.client_kind) }}</dd></div>
                                <div><dt>登录回调</dt><dd class="break-words">{{ uriList(form.development.redirect_uris, '未填写') }}</dd></div>
                                <div><dt>访问 Audience</dt><dd class="break-words" translate="no">{{ form.development.access_audience }}</dd></div>
                            </dl>
                            <UiButton size="sm" variant="outline" :disabled="pending" @click="goToStep(1)">修改开发环境</UiButton>
                        </section>
                        <section class="summary-card">
                            <h3>生产环境</h3>
                            <dl class="summary-list">
                                <div><dt>应用类型</dt><dd>{{ clientKindLabel(form.production.client_kind) }}</dd></div>
                                <div><dt>登录回调</dt><dd class="break-words">{{ uriList(form.production.redirect_uris, administrator ? '未填写' : '暂不填写，审批前补充') }}</dd></div>
                                <div><dt>访问 Audience</dt><dd class="break-words" translate="no">{{ form.production.access_audience }}</dd></div>
                            </dl>
                            <UiButton size="sm" variant="outline" :disabled="pending" @click="goToStep(2)">修改生产环境</UiButton>
                        </section>
                    </div>
                    <label v-if="administrator" class="checkbox-field">
                        <input id="accept-limitation" v-model="limitation" type="checkbox" :disabled="pending" :aria-invalid="!!fieldError('accept_logout_limitation')" />
                        <span>接受已记录的退出能力限制</span>
                    </label>
                    <p v-if="fieldError('accept_logout_limitation')" class="field-error" role="alert">{{ fieldError('accept_logout_limitation') }}</p>
                    <p class="form-help">提交后会立即创建应用、开发环境和生产环境草稿；服务端 Web 应用会在回执中展示一次性密钥。</p>
                </section>

                <p v-if="pending" class="form-help" role="status">正在建立应用…</p>
            </div>

            <footer class="registration-footer">
                <UiButton variant="ghost" type="button" :disabled="pending" @click="toggleRegistration">取消注册</UiButton>
                <div class="wizard-actions">
                    <UiButton v-if="stepIndex > 0" variant="outline" type="button" :disabled="pending" @click="previousStep">上一步</UiButton>
                    <UiButton v-if="stepIndex < registrationSteps.length - 1" type="button" :disabled="pending" @click="nextStep">下一步</UiButton>
                    <UiButton v-else type="submit" :disabled="pending" :loading="pending">{{ pending ? '正在建立应用…' : '确认并建立应用' }}</UiButton>
                </div>
            </footer>
        </form>

        <section class="panel">
            <div class="table-toolbar">
                <div class="search-box">
                    <Search :size="15" aria-hidden="true" />
                    <UiInput v-model="query" clearable placeholder="筛选当前页应用名称或 Key…" aria-label="筛选当前页应用" />
                </div>
                <span class="toolbar-spacer text-[11px] text-muted-foreground">当前页 {{ filteredApps.length }} 个应用</span>
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!data?.items.length"
                empty-title="还没有应用"
                :empty-description="administrator ? '配置第一个获批第三方应用后，会在这里显示。' : '注册第一个应用后，可以在这里查看开发与生产接入状态。'"
                :skeleton-rows="3"
                @retry="list.refresh()"
            >
                <div v-if="filteredApps.length" class="grid gap-4 p-4 md:grid-cols-2">
                    <RouterLink
                        v-for="app in filteredApps"
                        :key="app.application_id"
                        class="app-card"
                        :to="{ name: administrator ? 'admin-app' : 'developer-app', params: { id: app.application_id } }"
                    >
                        <div class="min-w-0 flex-1">
                            <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
                                <h2 class="min-w-0 truncate font-semibold">{{ app.name }}</h2>
                                <span class="status-badge" :class="app.active ? 'status-success' : 'status-muted'">{{ app.active ? '启用' : '停用' }}</span>
                            </div>
                            <p v-if="app.description" class="mt-2 line-clamp-2 text-xs text-muted-foreground">{{ app.description }}</p>
                            <p class="mt-2 text-xs text-muted-foreground">
                                <code class="break-all" translate="no">{{ app.app_key }}</code> · {{ app.management_mode === 'admin_configured' ? '管理员直接配置' : '开发者维护' }}
                            </p>
                            <ul class="app-environment-list">
                                <li v-for="env in app.environments" :key="env.environment_id">
                                    <span>{{ env.kind === 'development' ? '开发' : '生产' }}</span>
                                    <span class="status-badge" :class="env.pending_approval ? 'status-warning' : env.active_version_id ? 'status-success' : 'status-muted'">
                                        {{ env.pending_approval ? '待审批' : env.active_version_id ? '已有生效配置' : '尚未激活' }}
                                    </span>
                                </li>
                            </ul>
                        </div>
                        <ArrowRight :size="16" class="mt-1 shrink-0 text-muted-foreground/45" aria-hidden="true" />
                    </RouterLink>
                </div>
                <p v-else class="empty-filter">没有匹配“{{ query }}”的应用。</p>
                <div v-if="data?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多应用</UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
