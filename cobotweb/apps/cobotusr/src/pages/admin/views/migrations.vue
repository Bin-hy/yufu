<script setup lang="ts">
defineOptions({ name: 'AdminMigrations' })
import { reactive, ref } from 'vue'
import { ArrowRight, ScanSearch, ShieldCheck } from '@lucide/vue'
import { listMigrations, scanMigrations, adoptLegacy } from '../api'
import type { LegacyClient, Adoption } from '../api/type'
import { emptyConfig, type RegistrationReceipt } from '@/pages/developer/api/type'
import { validateRegistration } from '@/pages/developer/api/validation'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult, useOperationKey } from '@/composables/use-ephemeral-result'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import ConfigEditor from '@/pages/developer/components/config-editor.vue'
const list = useCursorPage('legacy-adoption', listMigrations),
    { data, status, error, refreshing } = list
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation,
    key = useOperationKey()
const selected = ref<LegacyClient>(),
    retained = ref<'development' | 'production'>('development'),
    assignmentIds = ref('')
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
    form.name = client.name
    form.app_key = ''
    form.owner = client.candidate_owner_id
    form.development = emptyConfig()
    form.production = emptyConfig()
    form.development.client_kind = form.production.client_kind = client.public_client
        ? 'browser_spa'
        : 'server_web'
}
async function adopt() {
    if (!selected.value) return
    const input: Adoption = {
        oauth_client_id: selected.value.oauth_client_id,
        expected_snapshot: selected.value.snapshot,
        existing_application_id: selected.value.candidate_application_ids[0] ?? null,
        retained_environment: retained.value,
        registration: {
            application: {
                app_key: form.app_key,
                name: form.name,
                description: form.description.trim() || null,
                third_party: form.third_party,
                development: form.development,
                production: form.production,
            },
            owner_user_id: form.owner,
            assignments: assignmentIds.value
                .split(',')
                .map((value) => value.trim())
                .filter(Boolean)
                .map((value) => ({ kind: 'user' as const, id: Number(value) })),
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
            await list.refresh()
            emitToast({ type: 'success', message: '历史客户端已纳入统一治理' })
        },
    )
}
function conflictTone(client: LegacyClient) {
    return client.conflict_kinds.length ? 'status-danger' : 'status-success'
}
const canAdopt = (client: LegacyClient) =>
    client.management_state !== 'managed' &&
    !client.conflict_kinds.some((value) => value !== 'orphan_client')
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="历史接入纳管"
            eyebrow="GOVERNANCE"
            description="扫描记录关联与冲突，明确选择负责人、保留环境及分配后纳入统一治理。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.first()">重新读取</UiButton>
            <UiButton :disabled="pending" :loading="pending" @click="scan">
                <ScanSearch v-if="!pending" :size="14" aria-hidden="true" />扫描首批历史关联
            </UiButton>
        </PageHeader>
        <p v-if="writeError" role="alert" class="form-alert mb-4">{{ writeError }}</p>

        <div class="inline-notice mb-5">
            <ShieldCheck :size="15" class="mt-0.5 shrink-0 text-primary" aria-hidden="true" />
            <p>
                纳管会保留选定环境的原 Client ID 与密钥，撤销旧授权，并为另一环境补建配置。执行前请确认负责人、保留环境与生产分配范围。
            </p>
        </div>

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
            <form class="surface-body space-y-6" @submit.prevent="adopt">
                <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                    <UiFormField id="adopt-key" label="应用 Key" required hint="已有应用必须保持原 Key">
                        <UiInput id="adopt-key" v-model="form.app_key" required :disabled="pending" placeholder="existing-app-key" />
                    </UiFormField>
                    <UiFormField id="adopt-name" label="应用名称" required>
                        <UiInput id="adopt-name" v-model="form.name" required :disabled="pending" />
                    </UiFormField>
                    <UiFormField id="adopt-owner" label="负责人账户 ID" required>
                        <UiInput id="adopt-owner" v-model.number="form.owner" type="number" min="1" required :disabled="pending" />
                    </UiFormField>
                    <UiFormField id="adopt-retained" label="保留历史客户端的环境">
                        <UiSelect id="adopt-retained" v-model="retained" :disabled="pending">
                            <option value="development">开发环境</option>
                            <option value="production">生产环境</option>
                        </UiSelect>
                    </UiFormField>
                    <UiFormField id="adopt-users" label="明确生产分配账户 ID" optional hint="逗号分隔，可留空">
                        <UiInput id="adopt-users" v-model="assignmentIds" :disabled="pending" placeholder="12, 18" />
                    </UiFormField>
                </div>
                <div class="grid gap-6 lg:grid-cols-2">
                    <section class="rounded-xl border border-border bg-muted/20 p-4">
                        <h3 class="mb-4 flex items-center gap-2 text-xs font-semibold">开发环境完整配置</h3>
                        <ConfigEditor v-model="form.development" environment="development" prefix="adopt-dev" administrator :disabled="pending" />
                    </section>
                    <section class="rounded-xl border border-border bg-muted/20 p-4">
                        <h3 class="mb-4 flex items-center gap-2 text-xs font-semibold">生产环境完整配置</h3>
                        <ConfigEditor v-model="form.production" environment="production" :activation="true" prefix="adopt-prod" administrator :disabled="pending" />
                    </section>
                </div>
                <label class="checkbox-field">
                    <input v-model="form.limitation" type="checkbox" :disabled="pending" />
                    <span>接受并记录退出能力限制</span>
                </label>
                <div class="surface-footer !-mx-5 !-mb-5 !mt-6">
                    <p class="form-help">提交后将立即执行纳管并创建补建环境。</p>
                    <div class="flex gap-2">
                        <UiButton variant="outline" type="button" :disabled="pending" @click="selected = undefined">取消</UiButton>
                        <UiButton type="submit" :disabled="pending" :loading="pending">按当前快照执行纳管</UiButton>
                    </div>
                </div>
            </form>
        </section>

        <section class="panel">
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!data?.items.length"
                empty-title="没有待纳管的历史接入"
                empty-description="运行扫描后，历史客户端会显示在这里。"
                :skeleton-rows="3"
                @retry="list.refresh()"
            >
                <div class="table-scroll desktop-table">
                    <table class="data-table">
                        <thead>
                            <tr><th>历史客户端</th><th>管理状态</th><th>关联应用</th><th>冲突</th><th class="text-right">操作</th></tr>
                        </thead>
                        <tbody>
                            <tr v-for="client in data?.items" :key="client.oauth_client_id">
                                <td>
                                    <p class="cell-primary">{{ client.name }}</p>
                                    <p class="cell-muted mt-0.5 mono">{{ client.client_id }}</p>
                                </td>
                                <td>{{ client.management_state }} · {{ client.public_client ? '公开客户端' : '机密客户端' }}</td>
                                <td>{{ client.candidate_application_ids.join('、') || '无' }}<p class="cell-muted mt-0.5">候选负责人 {{ client.candidate_owner_id }}</p></td>
                                <td><span class="status-badge" :class="conflictTone(client)">{{ client.conflict_kinds.join('、') || '无冲突' }}</span></td>
                                <td class="cell-actions">
                                    <UiButton variant="outline" size="sm" :disabled="pending || !canAdopt(client)" @click="select(client)">明确配置后纳管</UiButton>
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="mobile-list">
                    <article v-for="client in data?.items" :key="client.oauth_client_id" class="list-card">
                        <div class="min-w-0">
                            <div class="flex flex-wrap items-center gap-2">
                                <h3 class="list-title">{{ client.name }}</h3>
                                <span class="status-badge" :class="conflictTone(client)">{{ client.conflict_kinds.join('、') || '无冲突' }}</span>
                            </div>
                            <p class="list-meta mono">{{ client.client_id }}</p>
                            <p class="list-meta">候选负责人 {{ client.candidate_owner_id }} · 关联应用 {{ client.candidate_application_ids.join('、') || '无' }}</p>
                        </div>
                        <UiButton variant="outline" size="sm" :disabled="pending || !canAdopt(client)" @click="select(client)">明确配置后纳管</UiButton>
                    </article>
                </div>
                <div v-if="data?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多历史接入</UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
