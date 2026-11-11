<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { usePageContext } from '@/workspace/page-context'
import { useUserStore } from '@/pinia/stores/user'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult, useOperationKey } from '@/composables/use-ephemeral-result'
import { getApp, changeApp, getReaders, changeReader, getApproval, getCandidates, getAppDeliveries, getProviderMetadata } from '../api'
import { getAssignments, setAssignment, setTestUser } from '@/pages/admin/api'
import type { Environment, EnvironmentConfig, Reader } from '../api/type'
import { validateEnvironmentConfig } from '../api/validation'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import ConfigEditor from '../components/config-editor.vue'
const context = usePageContext(), user = useUserStore()
const provider = useAsyncQuery({ key: 'provider-metadata', active: context.active, loader: getProviderMetadata })
const administrator = computed(() => context.route.value.path.startsWith('/admin/')), id = computed(() => Number(context.route.value.params.id))
const state = useAsyncQuery({ key: () => `${administrator.value}:${id.value}`, active: context.active, loader: signal => getApp(administrator.value, id.value, signal) }), { data, status, error } = state
const selected = ref(''), config = ref<EnvironmentConfig>(), metadata = reactive({ name: '', description: '' }), candidate = ref(0), reason = ref(''), limitation = ref(false)
const env = computed(() => data.value?.environments.find(v => v.environment_id === selected.value))
const isOwner = computed(() => administrator.value || data.value?.owner_user_id === user.centerSession?.user_id)
watch(data, app => { metadata.name = app?.name ?? ''; metadata.description = app?.description ?? ''; if (app && !app.environments.some(e => e.environment_id === selected.value)) selected.value = app.environments[0]?.environment_id ?? '' })
watch(env, value => { config.value = value ? structuredClone(value.latest.config) : undefined }, { immediate: true })
const readers = useCursorPage(() => `readers:${id.value}:${selected.value}`, (after, signal) => getReaders(administrator.value, id.value, selected.value, signal, after), () => context.active.value && !!selected.value)
const deliveries = useCursorPage(() => `deliveries:${id.value}:${selected.value}`, (after, signal) => getAppDeliveries(administrator.value, id.value, selected.value, signal, after), () => context.active.value && !!selected.value)
const candidates = useAsyncQuery({ key: 'developer-candidates', active: () => context.active.value && !administrator.value && isOwner.value, loader: signal => getCandidates(signal) })
const extraCandidates = ref<{ user_id: number; username: string; nickname: string }[]>([]), candidateCursor = ref<number | null>(null), candidateBusy = ref(false)
const candidatePeople = computed(() => [...(candidates.data.value?.items ?? []), ...extraCandidates.value])
const candidateNext = computed(() => candidateCursor.value ?? candidates.data.value?.next ?? null)
watch(() => candidates.data.value, () => { extraCandidates.value = []; candidateCursor.value = null })
const deliveryStatusNames: Record<string, string> = { pending: '等待发送', leased: '发送处理中', retry: '等待重试', delivered: '已送达', permanent_failure: '发送失败', expired: '已超过期限' }
async function loadMoreCandidates() { const next = candidateNext.value; if (next == null || candidateBusy.value) return; candidateBusy.value = true; try { const page = await getCandidates(undefined, next); extraCandidates.value.push(...page.items); candidateCursor.value = page.next } finally { candidateBusy.value = false } }
const assignments = useAsyncQuery({ key: () => `assignments:${id.value}`, active: () => context.active.value && administrator.value, loader: signal => getAssignments(id.value, signal) })
const approval = useAsyncQuery({ key: () => `approval:${env.value?.pending_approval ?? ''}`, active: () => context.active.value && !!env.value?.pending_approval, loader: signal => getApproval(administrator.value, id.value, env.value!.pending_approval!, signal) })
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation, operationKey = useOperationKey(), secret = useEphemeralResult<{ label: string; value: string }>(), shownSecret = secret.value
const assignment = reactive({ kind: 'user' as 'user' | 'group', target: '', present: true }), testUser = ref(0)
const fieldLabels: Record<keyof EnvironmentConfig, string> = { client_kind: '客户端类型', redirect_uris: '登录回调', post_logout_redirect_uris: '退出回调', browser_origins: '浏览器来源', access_audience: '访问 Audience', identity_scopes: '资料范围', consent_policy: '同意策略', logout_capability: '退出能力', backchannel_logout_uri: '退出通知地址' }
type ConfigField = keyof EnvironmentConfig
interface ConfigChange { key: ConfigField; label: string; before: string; after: string }
function diffAgainst(before: EnvironmentConfig | null | undefined, after: EnvironmentConfig | null | undefined): ConfigChange[] {
    if (!after) return []
    return (Object.keys(fieldLabels) as ConfigField[])
        .filter(key => JSON.stringify(before?.[key]) !== JSON.stringify(after[key]))
        .map(key => ({ key, label: fieldLabels[key], before: JSON.stringify(before?.[key] ?? null), after: JSON.stringify(after[key]) }))
}
const changes = computed(() => (env.value && config.value ? diffAgainst(env.value.active_config, config.value) : []))
const approvalChanges = computed(() => {
    const fixed = approval.data.value?.config
    if (!fixed) return []
    const active = env.value?.active_config
    return (Object.keys(fieldLabels) as (keyof EnvironmentConfig)[]).filter(key => JSON.stringify(active?.[key] ?? null) !== JSON.stringify(fixed[key]))
        .map(key => ({ key, label: fieldLabels[key], before: JSON.stringify(active?.[key] ?? null), after: JSON.stringify(fixed[key]) }))
})
async function update(path: string, method: 'POST' | 'PUT' | 'PATCH', body: unknown) {
    await mutation.run(() => changeApp(administrator.value, id.value, path, method, body, operationKey({ id: id.value, path, body })), async receipt => {
        if (receipt.client_secret) shownSecret.value = { label: '客户端密钥（仅此次展示）', value: receipt.client_secret }
        await state.refresh(); readers.invalidate(); deliveries.invalidate(); assignments.invalidate(); approval.invalidate()
    })
}
const envPath = (environment: Environment) => `/environments/${environment.environment_id}`
async function saveConfig() {
    if (!env.value || !config.value) return
    const problem = validateEnvironmentConfig(config.value, env.value.kind, env.value.kind === 'development')
    if (problem) { writeError.value = problem; return }
    await update(`${envPath(env.value)}/config`, 'PUT', { expected_revision: env.value.revision, config: config.value })
}
async function submit() { if (env.value) await update(`${envPath(env.value)}/submit`, 'POST', { expected_revision: env.value.revision, version_id: env.value.latest.version_id }) }
async function activate() { if (env.value) await update(`${envPath(env.value)}/activate`, 'POST', { expected_revision: env.value.revision, version_id: env.value.latest.version_id, accept_logout_limitation: limitation.value }) }
async function review(approve: boolean) { if (env.value?.pending_approval) await update(`/approvals/${env.value.pending_approval}/review`, 'POST', { expected_revision: env.value.revision, approve, reason: reason.value.trim() || null, accept_logout_limitation: limitation.value }) }
async function readerChange(reader: Reader | null, reset: boolean) {
    if (!env.value?.active_config) return
    const body = reader ? { expected_revision: reader.revision, enabled: reset ? reader.enabled : !reader.enabled, reset_secret: reset } : { audience: env.value.active_config.access_audience }
    await mutation.run(() => changeReader(administrator.value, id.value, selected.value, reader?.reader_id ?? null, body, operationKey({ reader: reader?.reader_id, body })), async receipt => {
        if (receipt.secret) shownSecret.value = { label: `后端内省凭据 ${receipt.reader_id}`, value: receipt.secret }
        await readers.refresh()
    })
}
async function assign() {
    if (!data.value || !assignment.target.trim()) return
    const target = { kind: assignment.kind, id: assignment.kind === 'user' ? Number(assignment.target) : assignment.target.trim() }
    const body = { target, present: assignment.present, revision: data.value.revision }
    await mutation.run(() => setAssignment(id.value, target, assignment.present, data.value!.revision, operationKey(body)), async () => { await state.refresh(); await assignments.refresh() })
}
async function test(present: boolean) {
    if (!env.value || !data.value || !testUser.value) return
    const body = { id: id.value, env: selected.value, user: testUser.value, present, revision: data.value.revision }
    await mutation.run(() => setTestUser(id.value, selected.value, testUser.value, present, data.value!.revision, operationKey(body)), () => state.refresh())
}
</script>
<template>
    <div class="workspace-page"><PageHeader :title="data?.name || '应用接入'" description="开发和生产环境使用独立客户端。资料同意、生产访问分配与维护权分别管理。"><UiButton variant="outline" @click="state.refresh">重新读取</UiButton></PageHeader>
        <p v-if="writeError" role="alert" class="inline-error mb-4">{{ writeError }}</p>
        <section v-if="shownSecret" class="mb-5 rounded-xl border border-primary bg-card p-5"><h2>{{ shownSecret.label }}</h2><code class="my-3 block select-all break-all rounded bg-muted p-3 text-xs">{{ shownSecret.value }}</code><p class="text-xs text-muted-foreground">存入服务端秘密配置，离开页面后清除。</p><UiButton class="mt-3" variant="outline" @click="secret.clear">隐藏密钥</UiButton></section>
        <DataState :status="status" :error="error" @retry="state.refresh">
            <div v-if="data" class="space-y-6">
                <section class="rounded-xl border bg-card p-5"><h2 class="mb-3 font-semibold">应用资料与负责人</h2><form class="flex flex-wrap items-end gap-3" @submit.prevent="update('', 'PATCH', { name: metadata.name, description: metadata.description || null, expected_revision: data.revision })"><div class="form-field"><label for="detail-name">名称</label><UiInput id="detail-name" v-model="metadata.name" required :disabled="pending" /></div><div class="form-field"><label for="detail-description">说明</label><UiInput id="detail-description" v-model="metadata.description" :disabled="pending" /></div><UiButton type="submit" :disabled="pending">保存资料</UiButton></form><p class="mt-3 text-xs">负责人账户 {{ data.owner_user_id }} · {{ data.management_mode === 'admin_configured' ? '管理员直接配置' : '开发者维护' }}</p><div v-if="isOwner" class="mt-4 flex flex-wrap items-end gap-3"><div class="form-field"><label for="detail-candidate">候选账户</label><select v-if="!administrator" id="detail-candidate" v-model.number="candidate" class="ui-select"><option :value="0">选择已授权候选</option><option v-for="person in candidatePeople" :key="person.user_id" :value="person.user_id">{{ person.nickname || person.username }}（{{ person.user_id }}）</option></select><UiInput v-else id="detail-candidate" v-model.number="candidate" type="number" min="1" /></div><UiButton v-if="!administrator && candidateNext" variant="ghost" :disabled="candidateBusy" @click="loadMoreCandidates">加载更多候选账户</UiButton><UiButton variant="outline" :disabled="pending || !candidate" @click="update('/owner', 'PATCH', { user_id: candidate, expected_revision: data.revision })">交接负责人</UiButton><UiButton v-if="!administrator" variant="outline" :disabled="pending || !candidate" @click="update(`/members/${candidate}`, 'PATCH', { present: true, expected_revision: data.revision })">添加维护者</UiButton></div><p v-if="candidates.error.value" class="inline-error mt-3">{{ candidates.error.value }}</p><div class="mt-4 space-y-2"><div v-for="member in data.members" :key="member.user_id" class="flex items-center gap-3 text-sm">账户 {{ member.user_id }} · {{ member.member_role === 'owner' ? '负责人' : '维护者' }}<UiButton v-if="!administrator && isOwner && member.member_role !== 'owner'" variant="ghost" :disabled="pending" @click="update(`/members/${member.user_id}`, 'PATCH', { present: false, expected_revision: data.revision })">移除维护者</UiButton></div></div><UiButton v-if="administrator" class="mt-4" variant="outline" :disabled="pending" @click="update('/lifecycle', 'PATCH', { active: !data.active, expected_revision: data.revision })">{{ data.active ? '停用应用并撤销授权' : '恢复应用（重检负责人）' }}</UiButton></section>
                <section class="rounded-xl border bg-card p-5"><label for="detail-environment" class="mb-2 block text-sm font-semibold">应用环境</label><select id="detail-environment" v-model="selected" class="ui-select mb-5"><option v-for="environment in data.environments" :key="environment.environment_id" :value="environment.environment_id">{{ environment.kind === 'development' ? '开发环境' : '生产环境' }}</option></select><template v-if="env"><p class="text-sm">{{ env.pending_approval ? '生产申请待审核' : env.active_version_id ? '存在生效配置' : '尚未激活' }} · 最新版本 {{ env.latest.version }}</p><p class="mt-2 select-all break-all text-xs">Client ID：{{ env.client_id }}</p><p v-if="env.latest.candidate_client_id" class="mt-2 text-xs">类型切换候选 Client ID：{{ env.latest.candidate_client_id }}</p><div class="mt-5 grid gap-6 lg:grid-cols-2"><form @submit.prevent="saveConfig"><h2 class="mb-4 font-semibold">配置提案</h2><ConfigEditor v-if="config" v-model="config" prefix="detail-config" :environment="env.kind" :administrator="administrator" :disabled="pending" /><UiButton class="mt-4" type="submit" :disabled="pending">{{ env.kind === 'development' ? '保存并更新开发配置' : '保存生产配置提案' }}</UiButton></form><section><h2 class="mb-4 font-semibold">与生效配置的差异</h2><p v-if="!changes.length" class="text-sm text-muted-foreground">配置与生效版本一致。</p><dl v-for="change in changes" :key="change.key" class="mb-3 rounded bg-muted p-3 text-xs"><dt class="font-semibold">{{ change.label }}</dt><dd class="mt-2 break-all">当前：{{ change.before }}</dd><dd class="mt-2 break-all">提案：{{ change.after }}</dd></dl><template v-if="env.kind === 'production'"><UiButton v-if="!administrator" class="mt-4" :disabled="pending || !!env.pending_approval" @click="submit">提交已保存的最新版本供审核</UiButton><template v-else><label class="mt-4 flex gap-2 text-sm"><input v-model="limitation" type="checkbox">接受并记录退出能力限制</label><UiButton v-if="data.management_mode === 'admin_configured'" class="mt-3" :disabled="pending" @click="activate">激活已保存的生产版本</UiButton><template v-if="env.pending_approval"><p class="mt-4 text-sm">申请人账户 {{ approval.data.value?.requested_by }} · {{ approval.data.value?.status }}</p><div v-if="approval.data.value" class="mt-3 rounded border p-3"><h3 class="text-sm font-semibold">固定申请快照对比</h3><p class="mt-1 text-xs text-muted-foreground">审核对象为提交时固定的版本 {{ approval.data.value.version_id }}，不随后续编辑变化。</p><p v-if="!approvalChanges.length" class="mt-2 text-xs">与当前生效配置一致。</p><dl v-for="change in approvalChanges" :key="'approval-' + change.key" class="mb-2 rounded bg-muted p-2 text-xs"><dt class="font-semibold">{{ change.label }}</dt><dd class="mt-1 break-all">当前：{{ change.before }}</dd><dd class="mt-1 break-all">申请：{{ change.after }}</dd></dl><p class="mt-2 text-xs">快照负责人账户 {{ approval.data.value.snapshot.owner_user_id }} · 退出限制已接受：{{ approval.data.value.logout_limitation_accepted ? '是' : '否' }}</p></div><div class="form-field mt-3"><label for="review-reason">审批原因</label><UiInput id="review-reason" v-model="reason" :disabled="pending" /></div><div class="mt-3 flex gap-3"><UiButton :disabled="pending || approval.data.value?.requested_by === user.centerSession?.user_id" @click="review(true)">批准固定申请快照</UiButton><UiButton variant="outline" :disabled="pending" @click="review(false)">拒绝申请</UiButton></div><p v-if="approval.error.value" role="alert" class="inline-error mt-3">{{ approval.error.value }}</p></template></template></template></section></div><section class="mt-6 border-t pt-5"><h2 class="font-semibold">标准接入配置</h2><p class="mt-2 text-sm">Issuer：{{ provider.data.value?.issuer }} · Discovery：{{ provider.data.value?.issuer }}/.well-known/openid-configuration</p><p class="mt-2 text-xs text-muted-foreground">从 Discovery 获取协议端点。授权码流程要求 S256 PKCE、state 与 nonce；验证签名、issuer、audience。保护 API 在线内省缓存最长 5 秒。</p><p v-if="env.active_config" class="mt-2 text-sm">生效范围 {{ env.active_config.identity_scopes }} · Audience {{ env.active_config.access_audience }}</p><p v-else class="mt-2 text-sm">此环境尚无生效接入配置。</p><UiButton v-if="env.primary_client_kind === 'server_web'" class="mt-3" variant="outline" :disabled="pending" @click="update(`${envPath(env)}/secret/reset`, 'POST', { expected_revision: env.revision })">重置本环境客户端密钥</UiButton></section><section class="mt-6 border-t pt-5"><h2 class="mb-3 font-semibold">保护 API 内省凭据</h2><p class="mb-3 text-xs text-muted-foreground">仅交给此环境的后端 API；SPA 仍为公开客户端，浏览器不能保存该秘密。</p><UiButton :disabled="pending || !env.active_config" variant="outline" @click="readerChange(null, false)">为生效 Audience 创建后端凭据</UiButton><p v-if="readers.error.value" class="inline-error mt-3">{{ readers.error.value }}</p><article v-for="reader in readers.data.value?.items" :key="reader.id" class="mt-3 rounded border p-3"><p class="break-all text-xs">{{ reader.reader_id }} · {{ reader.enabled ? '启用' : '停用' }} · {{ reader.approved_audience }}</p><div class="mt-2 flex gap-3"><UiButton variant="ghost" :disabled="pending" @click="readerChange(reader, true)">重置秘密</UiButton><UiButton variant="ghost" :disabled="pending" @click="readerChange(reader, false)">{{ reader.enabled ? '停用' : '启用' }}</UiButton></div></article><UiButton v-if="readers.data.value?.next" class="mt-3" variant="outline" @click="readers.next">下一页凭据</UiButton></section></template></section>
                <section class="rounded-xl border bg-card p-5"><h2 class="mb-4 font-semibold">退出通知投递</h2><p class="text-xs text-muted-foreground">对端返回 2xx 只表示接受通知；本地授权撤销已在此之前生效，投递失败不影响本中心退出。</p><p v-if="deliveries.error.value" class="inline-error mt-3">{{ deliveries.error.value }}</p><article v-for="delivery in deliveries.data.value?.items" :key="delivery.id" class="mt-3 rounded border p-3"><p class="text-sm">{{ deliveryStatusNames[delivery.status] || delivery.status }} · 尝试 {{ delivery.attempt_count }} 次{{ delivery.last_error_code ? ' · ' + delivery.last_error_code : '' }}</p><p class="mt-1 text-xs text-muted-foreground">建立 {{ new Date(delivery.created_at * 1000).toLocaleString() }}{{ delivery.delivered_at ? ' · 送达 ' + new Date(delivery.delivered_at * 1000).toLocaleString() : '' }}</p></article><p v-if="deliveries.data.value && !deliveries.data.value.items.length" class="mt-3 text-sm text-muted-foreground">此环境尚无退出通知投递。</p><UiButton v-if="deliveries.data.value?.next" class="mt-3" variant="outline" @click="deliveries.next">下一页投递</UiButton></section>
                <section v-if="administrator" class="rounded-xl border bg-card p-5"><h2 class="mb-4 font-semibold">生产访问分配</h2><p v-if="assignments.error.value" class="inline-error">{{ assignments.error.value }}</p><p v-for="target in assignments.data.value" :key="target.kind + ':' + target.id" class="my-2 text-sm">{{ target.kind === 'user' ? '账户' : '用户组' }} {{ target.id }}</p><form class="mt-4 flex flex-wrap items-end gap-3" @submit.prevent="assign"><div class="form-field"><label for="assign-kind">对象类型</label><select id="assign-kind" v-model="assignment.kind" class="ui-select"><option value="user">账户</option><option value="group">用户组</option></select></div><div class="form-field"><label for="assign-id">对象 ID</label><UiInput id="assign-id" v-model="assignment.target" required /></div><label class="mb-2 flex gap-2 text-sm"><input v-model="assignment.present" type="checkbox">授予访问（取消为移除）</label><UiButton type="submit" :disabled="pending">更新明确分配</UiButton></form><div v-if="env?.kind === 'development'" class="mt-5 flex flex-wrap items-end gap-3"><div class="form-field"><label for="test-user">开发测试账户 ID</label><UiInput id="test-user" v-model.number="testUser" type="number" min="1" /></div><UiButton :disabled="pending || !testUser" @click="test(true)">添加测试访问</UiButton><UiButton variant="outline" :disabled="pending || !testUser" @click="test(false)">移除测试访问</UiButton></div></section>
            </div>
        </DataState>
    </div>
</template>
