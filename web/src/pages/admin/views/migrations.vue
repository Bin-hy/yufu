<script setup lang="ts">
defineOptions({ name: 'AdminMigrations' })
import { reactive, ref } from 'vue'
import { ArrowLeft, ArrowRight, ScanSearch } from '@lucide/vue'
import { listMigrations, scanMigrations, adoptLegacy, searchAccounts } from '../api'
import type { LegacyClient, Adoption } from '../api/type'
import { emptyConfig, type RegistrationReceipt } from '@/pages/developer/api/type'
import { validateRegistration } from '@/pages/developer/api/validation'
import { useCursorList } from '@/composables/use-cursor-list'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult, useOperationKey } from '@/composables/use-ephemeral-result'
import { emitToast } from '@/utils/toast'
import { conflictDescription, managementStateName } from '@/lib/identity-labels'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import ConfigEditor from '@/pages/developer/components/config-editor.vue'
import EntityPicker from '@/components/ui/entity-picker/entity-picker.vue'
import type { PickerOption } from '@/components/ui/entity-picker/types'
import { accountOption } from '@/lib/picker-options'
const list = useCursorList('legacy-adoption', listMigrations),
    { items, hasMore, status, error, refreshing } = list
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation,
    key = useOperationKey()
const selected = ref<LegacyClient>(),
    retained = ref<'development' | 'production'>('development'),
    assignmentOptions = ref<PickerOption[]>([]),
    assignmentPicker = ref<number | null>(null),
    ownerLabel = ref(''),
    ownerSecondary = ref('')
/** 纳管向导的三个阶段：归属 → 环境配置 → 分配与确认。 */
const steps = [
    { id: 'basics', title: '归属与保留' },
    { id: 'environments', title: '环境配置' },
    { id: 'confirm', title: '分配与确认' },
] as const
const step = ref(0),
    stepError = ref('')
const loadAccountOptions = async (
    query: string,
    signal: AbortSignal,
): Promise<PickerOption[]> => {
    const page = await searchAccounts(query, signal)
    return page.items.map(accountOption)
}
function chooseOwner(option: PickerOption) {
    form.owner = Number(option.value)
    ownerLabel.value = option.label
    ownerSecondary.value = option.secondary ?? ''
}
function addAssignment(option: PickerOption) {
    const id = Number(option.value)
    if (!assignmentOptions.value.some((entry) => Number(entry.value) === id))
        assignmentOptions.value.push({ ...option, value: id })
    assignmentPicker.value = null
}
function removeAssignment(id: number | string) {
    assignmentOptions.value = assignmentOptions.value.filter(
        (entry) => entry.value !== id,
    )
}
async function resolveOwner(id: number) {
    ownerLabel.value = `账户 ${id}`
    ownerSecondary.value = '正在读取账户信息…'
    try {
        const page = await searchAccounts(String(id))
        const account = page.items.find((item) => item.user_id === id)
        if (account) {
            ownerLabel.value = account.nickname || account.username
            ownerSecondary.value = `@${account.username} · #${account.user_id}`
        }
    } catch {
        ownerSecondary.value = ''
    }
}
const form = reactive({
    app_key: '',
    name: '',
    description: '',
    owner: 0,
    development: emptyConfig(),
    production: emptyConfig(),
    limitation: false,
    third_party: false,
})
const ephemeral = useEphemeralResult<RegistrationReceipt>(), receipt = ephemeral.value
async function scan() {
    await mutation.run(
        () => scanMigrations(),
        async () => {
            await list.refresh()
            emitToast({ type: 'success', message: '历史关联扫描已完成' })
        },
    )
}
function select(client: LegacyClient) {
    selected.value = client
    step.value = 0
    stepError.value = ''
    form.name = client.name
    form.app_key = ''
    form.owner = client.candidate_owner_id
    assignmentOptions.value = []
    void resolveOwner(client.candidate_owner_id)
    form.development = emptyConfig()
    form.production = emptyConfig()
    form.development.client_kind = form.production.client_kind = client.public_client
        ? 'browser_spa'
        : 'server_web'
}
function registration() {
    return {
        app_key: form.app_key,
        name: form.name,
        description: form.description.trim() || null,
        third_party: form.third_party,
        development: form.development,
        production: form.production,
    }
}
function back() {
    stepError.value = ''
    if (step.value > 0) step.value--
}
function forward() {
    stepError.value = ''
    if (step.value === 0) {
        if (!form.app_key.trim()) {
            stepError.value = '请填写历史应用标识（Key）。'
            document.getElementById('adopt-key')?.focus()
            return
        }
        if (!form.name.trim()) {
            stepError.value = '请填写应用名称。'
            document.getElementById('adopt-name')?.focus()
            return
        }
        if (!form.owner) {
            stepError.value = '请选择负责人账户。'
            return
        }
    }
    if (step.value === 1) {
        const problem = validateRegistration(registration(), true)
        if (problem) {
            stepError.value = problem
            return
        }
    }
    if (step.value < steps.length - 1) step.value++
}
async function adopt() {
    if (!selected.value) return
    const input: Adoption = {
        oauth_client_id: selected.value.oauth_client_id,
        expected_snapshot: selected.value.snapshot,
        existing_application_id: selected.value.candidate_application_ids[0] ?? null,
        retained_environment: retained.value,
        registration: {
            application: registration(),
            owner_user_id: form.owner,
            assignments: assignmentOptions.value.map((option) => ({
                kind: 'user' as const,
                id: Number(option.value),
            })),
            accept_logout_limitation: form.limitation,
        },
    }
    await mutation.run(
        () => {
            const problem = validateRegistration(input.registration.application, true)
            if (problem) throw new Error(problem)
            return adoptLegacy(input, key(input))
        },
        async (value) => {
            receipt.value = value
            selected.value = undefined
            assignmentOptions.value = []
            await list.refresh()
            emitToast({ type: 'success', message: '历史客户端已纳入统一治理' })
        },
    )
}
const blockers = (client: LegacyClient) =>
    client.conflict_kinds.filter((kind) => conflictDescription(kind).blocking)
const canAdopt = (client: LegacyClient) =>
    client.management_state !== 'managed' && !blockers(client).length
const adoptBlockedReason = (client: LegacyClient) =>
    client.management_state === 'managed'
        ? '该客户端已处于纳管状态。'
        : blockers(client).length
          ? `需先解决：${blockers(client).map((kind) => conflictDescription(kind).name).join('、')}`
          : ''
const conflictTone = (client: LegacyClient) =>
    blockers(client).length ? 'status-danger' : client.conflict_kinds.length ? 'status-warning' : 'status-success'
function conflictTitle(kind: string) {
    const description = conflictDescription(kind)
    return `${description.name}：${description.explanation}`
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="历史接入纳管"
            eyebrow="GOVERNANCE"
            hint="纳管保留选定环境原 Client ID 与密钥，撤销旧授权，并为另一环境补建配置。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.refresh()">刷新</UiButton>
            <UiButton :disabled="pending" :loading="pending" @click="scan">
                <ScanSearch v-if="!pending" :size="14" aria-hidden="true" />扫描历史接入
            </UiButton>
        </PageHeader>
        <p v-if="writeError" role="alert" class="form-alert mb-4">{{ writeError }}</p>

        <section v-if="receipt" class="panel mb-5 border-success/30">
            <header class="panel-heading bg-success/[0.04]">
                <div>
                    <h2>已纳管应用 {{ receipt.application_id }}</h2>
                    <p>操作回执 <span class="mono">{{ receipt.operation_id }}</span></p>
                </div>
                <span class="status-badge status-success">已完成</span>
            </header>
            <div class="surface-body space-y-3">
                <div v-for="secret in receipt.newly_issued_secrets" :key="secret.environment_id">
                    <p class="mb-1.5 text-[11px] font-semibold">补建环境密钥（仅此次展示）</p>
                    <code class="secret-value">{{ secret.client_secret }}</code>
                </div>
                <UiButton variant="outline" @click="ephemeral.clear()">隐藏回执与密钥</UiButton>
            </div>
        </section>

        <section v-if="selected" class="panel mb-5">
            <header class="panel-heading bg-muted/30">
                <div>
                    <h2 class="flex items-center gap-2"><ArrowRight :size="15" class="text-primary" aria-hidden="true" />纳管 {{ selected.name }}</h2>
                    <p class="mono">{{ selected.client_id }}</p>
                </div>
                <UiButton variant="ghost" @click="selected = undefined">取消</UiButton>
            </header>
            <form class="surface-body space-y-6" @submit.prevent="step === steps.length - 1 ? adopt() : forward()">
                <ol class="flex flex-wrap items-center gap-2 text-[11px]" aria-label="纳管步骤">
                    <li v-for="(entry, index) in steps" :key="entry.id" class="flex items-center gap-2">
                        <span
                            class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1"
                            :class="index === step ? 'border-primary bg-primary/10 font-semibold text-primary' : index < step ? 'border-success/40 text-success' : 'border-border text-muted-foreground'"
                            :aria-current="index === step ? 'step' : undefined"
                        >
                            {{ index + 1 }}. {{ entry.title }}
                        </span>
                        <span v-if="index < steps.length - 1" class="text-muted-foreground" aria-hidden="true">→</span>
                    </li>
                </ol>
                <p v-if="stepError" id="adopt-step-error" role="alert" class="form-alert">{{ stepError }}</p>

                <div v-if="step === 0" class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                    <UiFormField id="adopt-key" label="应用标识（历史 Key）" required hint="已有应用必须保留原标识，以维持原资源归属">
                        <UiInput id="adopt-key" v-model="form.app_key" :disabled="pending" placeholder="existing-app-key" />
                    </UiFormField>
                    <UiFormField id="adopt-name" label="应用名称" required>
                        <UiInput id="adopt-name" v-model="form.name" :disabled="pending" />
                    </UiFormField>
                    <UiFormField id="adopt-owner" label="负责人账户" required hint="按名称、用户名或账户 ID 搜索。">
                        <EntityPicker
                            id="adopt-owner"
                            :model-value="form.owner || null"
                            :selected-label="ownerLabel"
                            :selected-secondary="ownerSecondary"
                            :load="loadAccountOptions"
                            placeholder="搜索负责人姓名或用户名…"
                            :disabled="pending"
                            @update:model-value="(value) => { if (value === null) { form.owner = 0; ownerLabel = ''; ownerSecondary = '' } }"
                            @select="chooseOwner"
                        />
                    </UiFormField>
                    <UiFormField id="adopt-retained" label="保留历史客户端的环境" hint="保留环境沿用原 Client ID 与密钥，另一环境补建新配置。">
                        <UiSelect id="adopt-retained" v-model="retained" :disabled="pending">
                            <option value="development">开发环境</option>
                            <option value="production">生产环境</option>
                        </UiSelect>
                    </UiFormField>
                </div>

                <div v-else-if="step === 1" class="grid gap-6 lg:grid-cols-2">
                    <section class="rounded-xl border border-border bg-muted/20 p-4">
                        <h3 class="mb-4 flex items-center gap-2 text-xs font-semibold">开发环境完整配置</h3>
                        <ConfigEditor v-model="form.development" environment="development" prefix="adopt-dev" administrator :disabled="pending" />
                    </section>
                    <section class="rounded-xl border border-border bg-muted/20 p-4">
                        <h3 class="mb-4 flex items-center gap-2 text-xs font-semibold">生产环境完整配置</h3>
                        <ConfigEditor v-model="form.production" environment="production" :activation="true" prefix="adopt-prod" administrator :disabled="pending" />
                    </section>
                </div>

                <div v-else class="space-y-5">
                    <div class="grid gap-4 md:grid-cols-2">
                        <UiFormField id="adopt-users" label="明确生产分配账户" optional hint="搜索并逐个添加；留空表示暂不分配。">
                            <EntityPicker
                                id="adopt-users"
                                :model-value="assignmentPicker"
                                :load="loadAccountOptions"
                                placeholder="搜索要分配访问的账户…"
                                :disabled="pending"
                                @update:model-value="(value) => { assignmentPicker = value === null ? null : Number(value) }"
                                @select="addAssignment"
                            />
                            <div v-if="assignmentOptions.length" class="mt-2 flex flex-wrap gap-1.5">
                                <span v-for="option in assignmentOptions" :key="option.value" class="inline-flex items-center gap-1 rounded-md border border-border bg-muted/40 px-2 py-1 text-[10.5px]">
                                    {{ option.label }}
                                    <button type="button" class="ml-0.5 text-muted-foreground hover:text-destructive" :aria-label="'移除 ' + option.label" @click="removeAssignment(option.value)">×</button>
                                </span>
                            </div>
                        </UiFormField>
                        <div class="rounded-xl border border-border bg-muted/20 p-4 text-[11px] leading-relaxed">
                            <h3 class="mb-2 text-xs font-semibold">执行摘要</h3>
                            <p>应用标识 <span class="mono">{{ form.app_key }}</span> · {{ form.name }}</p>
                            <p>负责人：{{ ownerLabel || '账户 ' + form.owner }}</p>
                            <p>保留 {{ retained === 'development' ? '开发' : '生产' }}环境原客户端，另一环境补建新密钥。</p>
                            <p>生产分配：{{ assignmentOptions.length ? assignmentOptions.length + ' 个账户' : '暂不分配' }}</p>
                        </div>
                    </div>
                    <label class="checkbox-field">
                        <input v-model="form.limitation" type="checkbox" :disabled="pending" />
                        <span>接受并记录退出能力限制</span>
                    </label>
                </div>

                <div class="surface-footer !-mx-5 !-mb-5 !mt-6">
                    <p class="form-help">{{ step === steps.length - 1 ? '提交后将立即执行纳管并创建补建环境。' : `第 ${step + 1} 步，共 ${steps.length} 步。` }}</p>
                    <div class="flex gap-2">
                        <UiButton variant="outline" type="button" :disabled="pending" @click="selected = undefined">取消</UiButton>
                        <UiButton v-if="step > 0" variant="outline" type="button" :disabled="pending" @click="back">
                            <ArrowLeft :size="13" aria-hidden="true" />上一步
                        </UiButton>
                        <UiButton v-if="step < steps.length - 1" type="submit" :disabled="pending">下一步</UiButton>
                        <UiButton v-else type="submit" :disabled="pending" :loading="pending">按当前快照执行纳管</UiButton>
                    </div>
                </div>
            </form>
        </section>

        <section class="panel">
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!items.length"
                empty-title="没有待纳管的历史接入"
                empty-description="运行扫描后显示结果。"
                :skeleton-rows="3"
                @retry="list.refresh()"
            >
                <template #actions>
                    <UiButton :disabled="pending" :loading="pending" @click="scan">
                        <ScanSearch v-if="!pending" :size="14" aria-hidden="true" />扫描首批历史关联
                    </UiButton>
                </template>
                <div class="table-scroll desktop-table">
                    <table class="data-table">
                        <thead>
                            <tr><th>历史客户端</th><th>管理状态</th><th>关联应用</th><th>冲突</th><th class="text-right">操作</th></tr>
                        </thead>
                        <tbody>
                            <tr v-for="client in items" :key="client.oauth_client_id">
                                <td>
                                    <p class="cell-primary">{{ client.name }}</p>
                                    <p class="cell-muted mt-0.5 mono">{{ client.client_id }}</p>
                                </td>
                                <td>{{ managementStateName(client.management_state) }} · {{ client.public_client ? '公开客户端' : '机密客户端' }}</td>
                                <td>{{ client.candidate_application_ids.join('、') || '无' }}<p class="cell-muted mt-0.5">候选负责人 {{ client.candidate_owner_id }}</p></td>
                                <td>
                                    <div class="flex flex-wrap gap-1">
                                        <span v-if="!client.conflict_kinds.length" class="status-badge status-success">无冲突</span>
                                        <span
                                            v-for="kind in client.conflict_kinds"
                                            :key="kind"
                                            class="status-badge cursor-help"
                                            :class="conflictDescription(kind).blocking ? 'status-danger' : 'status-warning'"
                                            :title="conflictTitle(kind)"
                                        >{{ conflictDescription(kind).name }}</span>
                                    </div>
                                </td>
                                <td class="cell-actions">
                                    <span :title="adoptBlockedReason(client) || undefined">
                                        <UiButton variant="outline" size="sm" :disabled="pending || !canAdopt(client)" @click="select(client)">明确配置后纳管</UiButton>
                                    </span>
                                    <p v-if="adoptBlockedReason(client)" class="cell-muted mt-1 max-w-[200px] text-right">{{ adoptBlockedReason(client) }}</p>
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="mobile-list">
                    <article v-for="client in items" :key="client.oauth_client_id" class="list-card">
                        <div class="min-w-0">
                            <div class="flex flex-wrap items-center gap-2">
                                <h3 class="list-title">{{ client.name }}</h3>
                                <span class="status-badge" :class="conflictTone(client)">{{ client.conflict_kinds.length ? client.conflict_kinds.map(kind => conflictDescription(kind).name).join('、') : '无冲突' }}</span>
                            </div>
                            <p class="list-meta mono">{{ client.client_id }}</p>
                            <p class="list-meta">{{ managementStateName(client.management_state) }} · 候选负责人 {{ client.candidate_owner_id }} · 关联应用 {{ client.candidate_application_ids.join('、') || '无' }}</p>
                            <p v-if="adoptBlockedReason(client)" class="list-meta text-destructive">{{ adoptBlockedReason(client) }}</p>
                        </div>
                        <UiButton variant="outline" size="sm" :disabled="pending || !canAdopt(client)" @click="select(client)">明确配置后纳管</UiButton>
                    </article>
                </div>
                <div v-if="hasMore" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多历史接入</UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
