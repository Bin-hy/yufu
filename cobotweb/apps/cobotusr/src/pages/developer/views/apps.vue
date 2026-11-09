<script setup lang="ts">
defineOptions({ name: "DeveloperApps" })
import { computed, reactive, ref } from 'vue'
import { usePageContext } from '@/workspace/page-context'
import { useUserStore } from '@/pinia/stores/user'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useEphemeralResult, useOperationKey } from '@/composables/use-ephemeral-result'
import { listApps, registerApp, registerAdminApp } from '../api'
import { emptyConfig, type Registration, type RegistrationReceipt } from '../api/type'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import ConfigEditor from '../components/config-editor.vue'
const context = usePageContext(), user = useUserStore(), administrator = computed(() => context.route.value.path.startsWith('/admin/'))
const list = useCursorPage('managed-apps', (after: number | undefined, signal) => listApps(administrator.value, after, signal)), { data, status, error } = list
const opened = ref(false), owner = ref(user.centerSession?.user_id ?? 0), limitation = ref(false)
const form = reactive<Registration>({ app_key: '', name: '', description: '', third_party: false, development: emptyConfig(), production: emptyConfig() })
const result = useEphemeralResult<RegistrationReceipt>(), receipt = result.value
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation, operationKey = useOperationKey()
async function register() {
    const data: Registration = { ...form, description: form.description?.trim() || null }
    await mutation.run(() => administrator.value
        ? registerAdminApp({ application: data, owner_user_id: owner.value, assignments: [], accept_logout_limitation: limitation.value }, operationKey({ data, owner: owner.value, limitation: limitation.value }))
        : registerApp(data, operationKey(data)), async value => { receipt.value = value; opened.value = false; await list.refresh() })
}
</script>
<template>
    <div class="workspace-page"><PageHeader :title="administrator ? '企业应用' : '开发者控制台'" :description="administrator ? '审核生产接入、管理分配和直接配置获批第三方。' : '一次注册建立开发与生产环境，生产接入需独立审批。'"><UiButton @click="opened = !opened">{{ administrator ? '配置获批第三方' : '注册应用' }}</UiButton></PageHeader>
        <p v-if="writeError" role="alert" class="inline-error mb-4">{{ writeError }}</p>
        <section v-if="receipt" class="mb-6 rounded-xl border border-primary bg-card p-5"><h2 class="font-semibold">应用已建立</h2><p class="mt-2 text-sm">操作回执 {{ receipt.operation_id }}</p><div v-for="secret in receipt.secrets || receipt.newly_issued_secrets || []" :key="secret.environment_id" class="mt-3"><p class="text-xs">{{ receipt.environments.find(e => e.environment_id === secret.environment_id)?.kind === 'development' ? '开发环境' : '生产环境' }}密钥（仅此次展示）</p><code class="block select-all break-all rounded bg-muted p-3 text-xs">{{ secret.client_secret }}</code></div><p class="mt-3 text-xs text-muted-foreground">请保存到应用服务端的秘密配置中。离开页面后清除；响应丢失时可在环境详情重置密钥。</p><div class="mt-3 flex gap-3"><RouterLink class="ui-button ui-button-default" :to="{ name: administrator ? 'admin-app' : 'developer-app', params: { id: receipt.application_id } }">查看接入配置</RouterLink><UiButton variant="outline" @click="result.clear">隐藏回执与密钥</UiButton></div></section>
        <form v-if="opened" class="mb-6 rounded-xl border bg-card p-6" @submit.prevent="register"><div class="grid gap-4 md:grid-cols-2"><div class="form-field"><label for="app-key">应用 Key</label><UiInput id="app-key" v-model="form.app_key" required :disabled="pending" /></div><div class="form-field"><label for="app-name">应用名称</label><UiInput id="app-name" v-model="form.name" required :disabled="pending" /></div><div class="form-field"><label for="app-description">说明</label><UiInput id="app-description" v-model="form.description" :disabled="pending" /></div><div v-if="administrator" class="form-field"><label for="app-owner">明确负责人账户 ID</label><UiInput id="app-owner" v-model.number="owner" type="number" min="1" required :disabled="pending" /></div></div><label class="my-4 flex gap-2 text-sm"><input v-model="form.third_party" type="checkbox" :disabled="pending">第三方应用</label><div class="grid gap-6 lg:grid-cols-2"><section><h2 class="mb-4 font-semibold">开发环境</h2><ConfigEditor v-model="form.development" prefix="registration-dev" :administrator="administrator" :disabled="pending" /></section><section><h2 class="mb-4 font-semibold">生产环境</h2><ConfigEditor v-model="form.production" prefix="registration-prod" :administrator="administrator" :disabled="pending" /></section></div><label v-if="administrator" class="mt-5 flex gap-2 text-sm"><input v-model="limitation" type="checkbox" :disabled="pending">接受已记录的退出能力限制</label><UiButton class="mt-5" type="submit" :disabled="pending">{{ pending ? '正在建立…' : '建立应用与两环境' }}</UiButton></form>
        <DataState :status="status" :error="error" :empty="!data?.items.length" @retry="list.refresh"><div class="grid gap-4 md:grid-cols-2"><RouterLink v-for="app in data?.items" :key="app.application_id" class="rounded-xl border bg-card p-5 hover:border-primary" :to="{ name: administrator ? 'admin-app' : 'developer-app', params: { id: app.application_id } }"><h2 class="font-semibold">{{ app.name }} · {{ app.active ? '启用' : '停用' }}</h2><p class="mt-2 text-xs text-muted-foreground">{{ app.app_key }} · {{ app.management_mode === 'admin_configured' ? '管理员直接配置' : '开发者维护' }}</p><p v-for="env in app.environments" :key="env.environment_id" class="mt-2 text-sm">{{ env.kind === 'development' ? '开发' : '生产' }}：{{ env.pending_approval ? '待审批' : env.active_version_id ? '已有生效配置' : '尚未激活' }}</p></RouterLink></div></DataState><UiButton v-if="data?.next" class="mt-4" variant="outline" @click="list.next">下一页</UiButton>
    </div>
</template>
