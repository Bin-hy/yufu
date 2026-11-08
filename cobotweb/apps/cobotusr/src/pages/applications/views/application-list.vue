<script setup lang="ts">
import { computed, ref, shallowRef, onScopeDispose, nextTick } from 'vue'
import { Plus, Blocks, RefreshCw } from '@lucide/vue'
import * as appApi from '../api'
import * as oauthApi from '@/pages/oauth/api'
import type { ApplicationVO } from '@/types/models'
import type { ClientCreateResp } from '@/pages/oauth/api/type'
import { usePagedList } from '@/composables/use-paged-list'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import { createVerifiedCreate } from '@/composables/use-verified-create'
import { readAllPages } from '@/composables/use-paged-options'
import { createStepWorkflow } from '@/composables/use-step-workflow'
import { useTabsStore } from '@/pinia/stores/tabs'
import { sessionEpoch, onSessionReset } from '@/workspace/session'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import Editor, { type AppForm } from '../components/application-editor.vue'
import SecretResult from '../components/secret-result.vue'
const tabs = useTabsStore(),
    list = usePagedList((q, signal) =>
        appApi.getApplicationsByPage(q.page_number, q.page_size, {
            signal,
            errorPresentation: 'local',
        }),
    ),
    { page, size, totalPages, status, error, refreshing } = list
const removed = ref(new Set<number>()),
    rows = computed(() => list.rows.value.filter((app) => !removed.value.has(app.id)))
const clients = useAsyncQuery({
    key: 'oauth-clients',
    active: list.context.active,
    loader: (signal) => oauthApi.listClients({ signal, errorPresentation: 'local' }),
})
const scopeQuery = useAsyncQuery({
    key: 'oauth-scopes',
    active: list.context.active,
    loader: (signal) => oauthApi.listScopes({ signal, errorPresentation: 'local' }),
})
const scopes = computed(() => [
    ...new Set([
        'openid',
        'profile',
        'email',
        'phone',
        'address',
        'offline_access',
        ...(scopeQuery.data.value?.map((scope) => scope.name) ?? []),
        ...form.value.allowed_scopes,
    ]),
])
const clientFor = (app: ApplicationVO) => clients.data.value?.find((client) => client.id === app.oidc_client_id)
function defaults(): AppForm {
    return {
        name: '',
        description: '',
        redirect_uris: '',
        grant_types: ['authorization_code', 'refresh_token'],
        allowed_scopes: ['openid', 'profile', 'email'],
        public_client: false,
    }
}
const open = ref(false),
    editing = ref<ApplicationVO | null>(null),
    form = ref(defaults()),
    workflow = shallowRef<ReturnType<typeof createStepWorkflow> | null>(null),
    clientResult = shallowRef<ClientCreateResp | null>(null),
    validation = ref(''),
    invalidField = ref(''),
    done = ref(false)
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation
const draft = useFormDraft('应用配置', () => ({ form: form.value, steps: workflow.value?.completed.value ?? [] }), {
    enabled: open,
    submitting: pending,
    discard: clearEditor,
})
function clearEditor() {
    open.value = false
    workflow.value = null
    clientResult.value = null
    form.value = defaults()
    editing.value = null
    done.value = false
}
function edit(app?: ApplicationVO) {
    clearEditor()
    editing.value = app ?? null
    const client = app ? clientFor(app) : undefined
    if (app)
        form.value = {
            name: app.name,
            description: app.description ?? '',
            redirect_uris: client?.redirect_uris ?? '',
            grant_types: client?.grant_types.split(',').filter(Boolean) ?? ['authorization_code'],
            allowed_scopes: client?.allowed_scopes.split(',').filter(Boolean) ?? ['openid'],
            public_client: client?.public_client ?? false,
        }
    validation.value = ''
    invalidField.value = ''
    writeError.value = null
    draft.markSaved()
    open.value = true
}
async function beforeClose() {
    if (done.value) return clientResult.value?.client_secret ? confirmSecretSaved() : true
    if (workflow.value?.completed.value.length)
        return showModal({
            title: '关闭未完成的应用配置？',
            content:
                '已完成的步骤会保留在服务器；关闭后本次结果与重试进度会清除。客户端 ID：' +
                (clientResult.value?.client_id ?? editing.value?.app_key ?? ''),
            destructive: true,
            confirmText: '关闭配置',
        })
    return draft.confirmDiscard()
}
function confirmSecretSaved() {
    return showModal({
        title: '关闭密钥结果？',
        content: '请确认已保存客户端密钥。关闭或刷新页面后，本次密钥结果会丢失。',
        confirmText: '已保存，关闭',
        cancelText: '继续查看',
    })
}
function validate() {
    validation.value = ''
    invalidField.value = ''
    function invalid(field: string, message: string) {
        invalidField.value = field
        validation.value = message
        document.getElementById(field)?.focus()
        return false
    }
    const uris = form.value.redirect_uris
        .split(',')
        .map((uri) => uri.trim())
        .filter(Boolean)
    if (!form.value.name.trim() || !uris.length) {
        return invalid(
            !form.value.name.trim() ? 'application-name' : 'application-callback',
            '请填写应用名称和回调地址',
        )
    }
    if (!form.value.grant_types.length) {
        return invalid('application-grants', '至少选择一个授权类型')
    }
    try {
        for (const uri of uris) {
            const value = new URL(uri)
            if (!['https:', 'http:'].includes(value.protocol)) throw Error()
        }
    } catch {
        return invalid('application-callback', '回调地址应为完整的 HTTP 或 HTTPS 地址')
    }
    return true
}
async function save() {
    if (done.value) {
        clearEditor()
        return
    }
    if (!workflow.value) {
        if (!validate()) return
        const value = JSON.parse(JSON.stringify(form.value)) as AppForm,
            uris = value.redirect_uris
                .split(',')
                .map((uri) => uri.trim())
                .filter(Boolean),
            app = editing.value,
            epoch = sessionEpoch.value
        let originIds: Set<number> | null = null
        const clientPayload = {
            name: value.name.trim(),
            redirect_uri: uris[0]!,
            redirect_uris: uris,
            allowed_scopes: value.allowed_scopes,
            grant_types: value.grant_types,
            public_client: value.public_client,
        }
        const verifiedClient = createVerifiedCreate<ClientCreateResp>(
            async () => {
                return oauthApi.createClient(clientPayload, { errorPresentation: 'local' })
            },
            async () => {
                if (!originIds) return null
                const matches = (await oauthApi.listClients({ errorPresentation: 'local' })).filter(
                    (client) =>
                        !originIds!.has(client.id) &&
                        client.name === value.name.trim() &&
                        client.redirect_uris
                            .split(',')
                            .map((uri) => uri.trim())
                            .join(',') === uris.join(',') &&
                        client.public_client === value.public_client,
                )
                if (matches.length !== 1) return null
                validation.value = value.public_client
                    ? '客户端创建结果已核对。'
                    : '客户端创建结果已核对，但一次性密钥未收到。注册完成后请重新生成密钥。'
                return { ...matches[0]!, client_secret: null }
            },
        )
        const verifiedApplication = createVerifiedCreate(
            () =>
                appApi.createApplication(
                    {
                        app_key: clientResult.value!.client_id,
                        name: value.name.trim(),
                        description: value.description.trim() || null,
                        oidc_client_id: clientResult.value!.id,
                    },
                    { errorPresentation: 'local' },
                ),
            async () => {
                const controller = new AbortController()
                const rows = await readAllPages(
                    (p, size) =>
                        appApi.getApplicationsByPage(p, size, {
                            signal: controller.signal,
                            errorPresentation: 'local',
                        }),
                    controller.signal,
                )
                const matches = rows.filter(
                    (row) =>
                        row.app_key === clientResult.value!.client_id && row.oidc_client_id === clientResult.value!.id,
                )
                return matches.length === 1 ? matches[0]! : null
            },
        )
        workflow.value = createStepWorkflow(
            app
                ? [
                      {
                          id: 'application',
                          label: '更新应用资料',
                          run: () =>
                              appApi.patchApplication(
                                  {
                                      id: app.id,
                                      name: value.name.trim(),
                                      description: value.description.trim() || null,
                                  },
                                  { errorPresentation: 'local' },
                              ),
                      },
                      ...(app.oidc_client_id
                          ? [
                                {
                                    id: 'client',
                                    label: '更新 OAuth 客户端',
                                    run: () =>
                                        oauthApi.updateClient(app.oidc_client_id!, clientPayload, {
                                            errorPresentation: 'local',
                                        }),
                                },
                            ]
                          : []),
                  ]
                : [
                      {
                          id: 'client',
                          label: '创建 OAuth 客户端',
                          run: async () => {
                              if (!originIds)
                                  originIds = new Set(
                                      (await oauthApi.listClients({ errorPresentation: 'local' })).map(
                                          (client) => client.id,
                                      ),
                                  )
                              const result = await verifiedClient.run()
                              if (epoch === sessionEpoch.value) clientResult.value = result
                              return result
                          },
                      },
                      { id: 'application', label: '建立应用关联', run: verifiedApplication.run },
                  ],
        )
    }
    await mutation.run(
        () => workflow.value!.run(),
        async () => {
            done.value = true
            draft.markSaved()
            tabs.invalidate('permission-list')
            emitToast({ type: 'success', message: editing.value ? '应用已更新' : '应用注册成功' })
            await Promise.all([list.refresh(), clients.refresh()])
            if (!clientResult.value?.client_secret && !validation.value) clearEditor()
        },
    )
}
function remove(app: ApplicationVO) {
    const deletion = createStepWorkflow([
        ...(app.oidc_client_id
            ? [
                  {
                      id: 'client',
                      label: '删除 OAuth 客户端',
                      run: async () => {
                          try {
                              await oauthApi.deleteClient(app.oidc_client_id!, {
                                  errorPresentation: 'local',
                              })
                          } catch (error) {
                              if ((error as { response?: { status: number } }).response?.status !== 404) throw error
                          }
                      },
                  },
              ]
            : []),
        {
            id: 'application',
            label: '删除应用资料',
            run: () => appApi.deleteApplication(app.id, { errorPresentation: 'local' }),
        },
    ])
    void showModal({
        title: '删除应用？',
        content: '应用「' + app.name + '」及其 OAuth 客户端会被删除。部分失败后可在此重试剩余步骤。',
        destructive: true,
        confirmText: '删除应用',
        confirmCallback: async () => {
            await deletion.run()
            removed.value.add(app.id)
            tabs.invalidate('permission-list')
            emitToast({ type: 'success', message: '应用已删除' })
            await Promise.all([list.refresh(), clients.refresh()])
        },
    })
}
const secret = ref(''),
    secretClient = ref(''),
    secretOpen = ref(false)
async function regenerate(app: ApplicationVO) {
    if (!app.oidc_client_id) return
    const epoch = sessionEpoch.value
    const approved = await showModal({
        title: '重新生成客户端密钥？',
        content: '「' + app.name + '」的旧密钥将立即失效。',
        destructive: true,
        confirmText: '重新生成',
        confirmCallback: async () => {
            const value = await oauthApi.regenerateSecret(app.oidc_client_id!, {
                errorPresentation: 'local',
            })
            secret.value = value.client_secret
            secretClient.value = app.app_key
        },
    })
    if (approved && epoch === sessionEpoch.value) {
        await nextTick()
        secretOpen.value = true
    }
}
function closeSecret() {
    secretOpen.value = false
    secret.value = ''
}
async function copy(text: string) {
    try {
        await navigator.clipboard.writeText(text)
        emitToast({ type: 'success', message: 'Client ID 已复制' })
    } catch {
        emitToast({ type: 'error', message: '复制失败，请手动选择并复制 Client ID' })
    }
}
const cleanup = onSessionReset(() => {
    clearEditor()
    secret.value = ''
    secretOpen.value = false
})
onScopeDispose(() => {
    cleanup()
    clientResult.value = null
    secret.value = ''
})
async function refresh() {
    await Promise.all([list.refresh(), clients.refresh(), scopeQuery.refresh()])
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader title="应用注册" description="配置接入应用、OAuth 客户端与授权范围。" eyebrow="APPLICATIONS"
            ><template #actions
                ><UiButton variant="outline" :disabled="refreshing" @click="refresh"
                    ><RefreshCw :size="15" aria-hidden="true" />刷新</UiButton
                ><UiButton @click="edit()"><Plus :size="16" aria-hidden="true" />注册应用</UiButton></template
            ></PageHeader
        >
        <p v-if="clients.error.value" class="inline-error mb-4" role="alert">
            OAuth 客户端加载失败，应用资料仍可查看。{{ clients.error.value }}
            <UiButton variant="ghost" size="sm" @click="clients.refresh">重试</UiButton>
        </p>
        <div class="panel">
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!rows.length"
                @retry="list.refresh"
                ><div class="divide-y divide-border">
                    <article
                        v-for="app in rows"
                        :key="app.id"
                        class="p-5 flex flex-col sm:flex-row gap-4 sm:items-center"
                    >
                        <div
                            class="h-11 w-11 bg-secondary text-primary rounded-lg flex items-center justify-center shrink-0"
                        >
                            <Blocks :size="21" aria-hidden="true" />
                        </div>
                        <div class="min-w-0 flex-1">
                            <h2 class="font-semibold">{{ app.name }}</h2>
                            <p v-if="app.description" class="text-xs text-muted-foreground mt-1">
                                {{ app.description }}
                            </p>
                            <div class="flex flex-wrap items-center gap-2 mt-2">
                                <code class="text-xs break-all select-all text-muted-foreground">{{ app.app_key }}</code
                                ><UiButton size="sm" variant="ghost" @click="copy(app.app_key)">复制 ID</UiButton
                                ><span v-if="clientFor(app)?.public_client" class="status-badge">Public · PKCE</span>
                            </div>
                            <p v-if="clientFor(app)" class="text-xs text-muted-foreground break-all mt-1">
                                回调：{{ clientFor(app)?.redirect_uris }}
                            </p>
                            <p v-else class="text-xs text-muted-foreground mt-1">
                                {{ clients.status.value === 'loading' ? '正在获取客户端配置…' : '客户端配置暂不可用' }}
                            </p>
                        </div>
                        <div class="page-actions">
                            <UiButton
                                v-if="clientFor(app) && !clientFor(app)?.public_client"
                                size="sm"
                                variant="ghost"
                                @click="regenerate(app)"
                                >重置密钥</UiButton
                            ><UiButton
                                size="sm"
                                variant="outline"
                                :disabled="!!app.oidc_client_id && !clientFor(app)"
                                @click="edit(app)"
                                >编辑</UiButton
                            ><UiButton size="sm" variant="ghost" class="text-destructive" @click="remove(app)"
                                >删除</UiButton
                            >
                        </div>
                    </article>
                </div></DataState
            ><Pagination
                :page="page"
                :size="size"
                :page-count="totalPages"
                @page-change="list.setPage"
                @update:size="list.setSize"
            />
        </div>
        <UiDialog
            v-model:open="open"
            :title="done ? '应用注册完成' : editing ? '编辑应用' : '注册应用'"
            size="lg"
            :busy="pending"
            :error="writeError"
            :before-close="beforeClose"
            :confirm-text="done ? '已保存，完成' : workflow ? '重试剩余步骤' : '保存应用'"
            @confirm="save"
            @cancel="clearEditor"
            ><p v-if="scopeQuery.error.value" class="inline-error mb-4" role="alert">
                Scope 列表加载失败。<UiButton variant="ghost" size="sm" @click="scopeQuery.refresh">重试</UiButton>
            </p>
            <p
                v-if="validation"
                id="application-validation"
                :role="done ? 'status' : 'alert'"
                class="inline-error mb-4"
            >
                {{ validation }}
            </p>
            <Editor
                v-if="!done"
                v-model="form"
                :invalid-field="invalidField"
                :disabled="pending || !!workflow"
                :public-locked="!!editing && !!clientFor(editing)?.public_client"
                :scopes="scopes" />
            <ol v-if="workflow" class="my-4 space-y-2 text-xs" aria-label="应用保存进度">
                <li
                    v-for="step in workflow.steps"
                    :key="step.id"
                    :class="workflow.completed.value.includes(step.id) ? 'text-primary' : 'text-muted-foreground'"
                >
                    {{ workflow.completed.value.includes(step.id) ? '✓' : '○' }} {{ step.label }}
                </li>
            </ol>
            <SecretResult
                v-if="clientResult?.client_secret"
                :secret="clientResult.client_secret"
                :client-id="clientResult.client_id" /></UiDialog
        ><UiDialog
            v-model:open="secretOpen"
            title="新的客户端密钥"
            hide-footer
            :before-close="confirmSecretSaved"
            @cancel="secret = ''"
            ><SecretResult v-if="secret" :secret="secret" :client-id="secretClient" /><UiButton
                class="mt-4"
                @click="closeSecret"
                >已保存，关闭</UiButton
            ></UiDialog
        >
    </section>
</template>
