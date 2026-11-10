<script setup lang="ts">
defineOptions({ name: "AdminMigrations" })
import { reactive, ref } from 'vue'
import { listMigrations, scanMigrations, adoptLegacy } from '../api'
import type { LegacyClient, Adoption } from '../api/type'
import { emptyConfig, type RegistrationReceipt } from '@/pages/developer/api/type'
import { validateRegistration } from '@/pages/developer/api/validation'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult, useOperationKey } from '@/composables/use-ephemeral-result'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import ConfigEditor from '@/pages/developer/components/config-editor.vue'
const list = useCursorPage('legacy-adoption', listMigrations), { data, status, error } = list
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation, key = useOperationKey()
const selected = ref<LegacyClient>(), retained = ref<'development' | 'production'>('development'), assignmentIds = ref('')
const form = reactive({ app_key: '', name: '', description: '', owner: 0, development: emptyConfig(), production: emptyConfig(), limitation: false, third_party: false })
const ephemeral = useEphemeralResult<RegistrationReceipt>(), receipt = ephemeral.value
async function scan() { await mutation.run(() => scanMigrations(), () => list.refresh()) }
function select(client: LegacyClient) {
    selected.value = client; form.name = client.name; form.app_key = ''; form.owner = client.candidate_owner_id
    form.development = emptyConfig(); form.production = emptyConfig()
    form.development.client_kind = form.production.client_kind = client.public_client ? 'browser_spa' : 'server_web'
}
async function adopt() {
    if (!selected.value) return
    const input: Adoption = { oauth_client_id: selected.value.oauth_client_id, expected_snapshot: selected.value.snapshot, existing_application_id: selected.value.candidate_application_ids[0] ?? null, retained_environment: retained.value,
        registration: { application: { app_key: form.app_key, name: form.name, description: form.description.trim() || null, third_party: form.third_party, development: form.development, production: form.production }, owner_user_id: form.owner,
            assignments: assignmentIds.value.split(',').map(v => v.trim()).filter(Boolean).map(v => ({ kind: 'user', id: Number(v) })), accept_logout_limitation: form.limitation } }
    await mutation.run(() => {
        const problem = validateRegistration(input.registration.application, true)
        if (problem) throw new Error(problem)
        return adoptLegacy(input, key(input))
    }, async value => { receipt.value = value; selected.value = undefined; await list.refresh() })
}
</script>
<template>
    <div class="workspace-page"><PageHeader title="历史接入纳管" description="扫描记录关联与冲突，明确选择负责人、保留环境及分配后纳入统一治理。"><UiButton :disabled="pending" @click="scan">扫描首批历史关联</UiButton><UiButton variant="outline" @click="list.first">重新读取</UiButton></PageHeader><p v-if="writeError" class="inline-error mb-4" role="alert">{{ writeError }}</p>
        <section v-if="receipt" class="mb-5 rounded-xl border border-primary bg-card p-5"><h2>已纳管应用 {{ receipt.application_id }}</h2><p class="mt-2 text-xs">操作回执 {{ receipt.operation_id }}</p><div v-for="secret in receipt.newly_issued_secrets" :key="secret.environment_id" class="mt-3"><p class="text-xs">补建环境密钥（仅此次展示）</p><code class="block select-all break-all rounded bg-muted p-3 text-xs">{{ secret.client_secret }}</code></div><UiButton class="mt-3" variant="outline" @click="ephemeral.clear">隐藏回执与密钥</UiButton></section>
        <form v-if="selected" class="mb-5 rounded-xl border bg-card p-5" @submit.prevent="adopt"><h2 class="mb-4 font-semibold">纳管 {{ selected.name }} · {{ selected.client_id }}</h2><p class="mb-4 text-sm text-muted-foreground">保留选定历史 Client ID 与原密钥，撤销旧授权；另一环境会明确补建。</p><div class="mb-5 grid gap-4 md:grid-cols-2"><div class="form-field"><label for="adopt-key">应用 Key（已有应用须保持原 Key）</label><UiInput id="adopt-key" v-model="form.app_key" required /></div><div class="form-field"><label for="adopt-name">应用名称</label><UiInput id="adopt-name" v-model="form.name" required /></div><div class="form-field"><label for="adopt-owner">负责人账户 ID</label><UiInput id="adopt-owner" v-model.number="form.owner" type="number" min="1" required /></div><div class="form-field"><label for="adopt-retained">保留历史客户端的环境</label><select id="adopt-retained" v-model="retained" class="ui-select"><option value="development">开发环境</option><option value="production">生产环境</option></select></div><div class="form-field"><label for="adopt-users">明确生产分配账户 ID（逗号分隔，可空）</label><UiInput id="adopt-users" v-model="assignmentIds" /></div></div><div class="grid gap-6 lg:grid-cols-2"><section><h3 class="mb-4 font-semibold">开发环境完整配置</h3><ConfigEditor v-model="form.development" environment="development" prefix="adopt-dev" administrator :disabled="pending" /></section><section><h3 class="mb-4 font-semibold">生产环境完整配置</h3><ConfigEditor v-model="form.production" environment="production" :activation="true" prefix="adopt-prod" administrator :disabled="pending" /></section></div><label class="mt-5 flex gap-2 text-sm"><input v-model="form.limitation" type="checkbox">接受并记录退出能力限制</label><div class="mt-4 flex gap-3"><UiButton type="submit" :disabled="pending">按当前快照执行纳管</UiButton><UiButton variant="outline" @click="selected = undefined">取消</UiButton></div></form>
        <DataState :status="status" :error="error" :empty="!data?.items.length" @retry="list.refresh"><div class="space-y-3"><article v-for="client in data?.items" :key="client.oauth_client_id" class="flex flex-wrap items-center justify-between gap-4 rounded-xl border bg-card p-5"><div><h2 class="font-semibold">{{ client.name }}</h2><p class="mt-2 break-all text-xs">{{ client.client_id }} · {{ client.management_state }} · {{ client.public_client ? '公开客户端' : '机密客户端' }}</p><p class="mt-2 text-xs">关联应用 {{ client.candidate_application_ids.join('、') || '无' }} · 候选负责人 {{ client.candidate_owner_id }}</p><p v-if="client.conflict_kinds.length" class="mt-2 text-sm text-destructive">冲突：{{ client.conflict_kinds.join('、') }}</p></div><UiButton variant="outline" :disabled="pending || client.management_state === 'managed' || client.conflict_kinds.some(v => v !== 'orphan_client')" @click="select(client)">明确配置后纳管</UiButton></article></div></DataState><UiButton v-if="data?.next" class="mt-4" variant="outline" @click="list.next">下一页</UiButton>
    </div>
</template>
