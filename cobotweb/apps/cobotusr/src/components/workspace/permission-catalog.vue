<script setup lang="ts">
import { computed, ref, reactive } from 'vue'
import { Plus, RefreshCw } from '@lucide/vue'
import * as authApi from '@/pages/authority/api'
import * as permApi from '@/pages/permissions/api'
import * as appApi from '@/pages/applications/api'
import { usePagedList } from '@/composables/use-paged-list'
import { usePagedOptions } from '@/composables/use-paged-options'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import { useTabsStore } from '@/pinia/stores/tabs'
import type { AuthorityVO, PermissionVO } from '@/types/models'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import PageHeader from './page-header.vue'
import DataState from './data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
const props = defineProps<{ kind: 'authority' | 'permission' }>(),
    tabs = useTabsStore()
const list = usePagedList<AuthorityVO | PermissionVO>((q, signal) =>
    props.kind === 'authority'
        ? authApi.getAuthoritiesByPage(q.page_number, q.page_size, {
              signal,
              errorPresentation: 'local',
          })
        : permApi.getPermissionsByPage(q.page_number, q.page_size, q.application_id, {
              signal,
              errorPresentation: 'local',
          }),
)
const { page, size, status, error, refreshing, totalPages, query } = list,
    removed = ref(new Set<number>()),
    rows = computed(() => list.rows.value.filter((row) => !removed.value.has(row.id)))
const applications = usePagedOptions(
    (p, s, signal) => appApi.getApplicationsByPage(p, s, { signal, errorPresentation: 'local' }),
    () => props.kind + ':' + (tabs.invalidations['permission-list'] ?? 0),
    { active: () => props.kind === 'permission' && list.context.active.value },
)
const open = ref(false),
    editing = ref<number | null>(null),
    validation = ref(''),
    invalidField = ref(''),
    form = reactive({
        name: '',
        authority_uri: '',
        method: 'GET',
        application_id: 0,
        description: '',
        action: '',
        resource_type: '',
        resource_pattern: '',
    })
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation
const draft = useFormDraft('权限编辑', () => ({ ...form, name: form.name.trim() }), {
    enabled: open,
    submitting: pending,
    discard: () => {
        open.value = false
    },
})
const fields = [
    { key: 'name' as const, label: '权限名称', placeholder: '例如：查看用户' },
    { key: 'description' as const, label: '描述', placeholder: '可选说明' },
    { key: 'action' as const, label: '动作', placeholder: '例如：read' },
    { key: 'resource_type' as const, label: '资源类型', placeholder: '例如：document' },
    { key: 'resource_pattern' as const, label: '资源匹配', placeholder: '例如：document:*' },
]
function edit(row?: PermissionVO) {
    editing.value = row?.id ?? null
    Object.assign(form, {
        name: row?.name ?? '',
        authority_uri: '',
        method: 'GET',
        application_id: row?.application_id ?? query.value.application_id ?? applications.items.value[0]?.id ?? 0,
        description: row?.description ?? '',
        action: row?.action ?? '',
        resource_type: row?.resource_type ?? '',
        resource_pattern: row?.resource_pattern ?? '',
    })
    validation.value = ''
    invalidField.value = ''
    writeError.value = null
    draft.markSaved()
    open.value = true
}
async function save() {
    validation.value = ''
    invalidField.value = ''
    function invalid(field: string, message: string) {
        invalidField.value = field
        validation.value = message
        document.getElementById(field)?.focus()
    }
    if (!form.name.trim()) {
        invalid(props.kind === 'authority' ? 'authority-name' : 'permission-name', '请填写权限名称')
        return
    }
    if (props.kind === 'authority' && !form.authority_uri.trim()) {
        invalid('authority-uri', '请填写权限标识')
        return
    }
    if (props.kind === 'permission' && (!form.application_id || !form.action.trim())) {
        invalid(!form.application_id ? 'permission-app' : 'permission-action', '请选择应用并填写动作')
        return
    }
    await mutation.run<AuthorityVO | PermissionVO>(
        () => {
            const options = { errorPresentation: 'local' as const }
            if (props.kind === 'authority')
                return authApi.createAuthority(
                    {
                        name: form.name.trim(),
                        authority_uri: form.authority_uri.trim(),
                        method: form.method,
                    },
                    options,
                )
            const payload = {
                name: form.name.trim(),
                description: form.description.trim() || null,
                application_id: form.application_id,
                action: form.action.trim(),
                resource_type: form.resource_type.trim() || null,
                resource_pattern: form.resource_pattern.trim() || null,
            }
            return editing.value === null
                ? permApi.createPermission(payload, options)
                : permApi.patchPermission({ id: editing.value, ...payload }, options)
        },
        async () => {
            draft.markSaved()
            open.value = false
            tabs.invalidate(props.kind === 'authority' ? 'authority-assign' : 'permission-assign')
            emitToast({ type: 'success', message: '权限已保存' })
            await list.refresh()
        },
    )
}
function remove(row: PermissionVO) {
    void showModal({
        title: '删除资源权限？',
        content: '权限「' + row.name + '」将被删除。',
        destructive: true,
        confirmText: '删除权限',
        confirmCallback: async () => {
            await permApi.deletePermission(row.id, { errorPresentation: 'local' })
            removed.value.add(row.id)
            tabs.invalidate('permission-assign')
            emitToast({ type: 'success', message: '权限已删除' })
            await list.refresh()
        },
    })
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader
            :title="kind === 'authority' ? 'API 权限' : '资源权限'"
            :description="kind === 'authority' ? '维护接口标识与请求方法。' : '管理应用资源的动作与匹配规则。'"
            eyebrow="ACCESS CONTROL"
            ><template #actions
                ><RouterLink
                    :to="{ name: kind === 'authority' ? 'authority-assign' : 'permission-assign' }"
                    class="ui-button ui-button-outline"
                    >分配权限</RouterLink
                ><UiButton @click="edit()"><Plus :size="16" aria-hidden="true" />新增权限</UiButton></template
            ></PageHeader
        >
        <div class="panel">
            <div class="query-toolbar">
                <div v-if="kind === 'permission'" class="flex flex-wrap gap-2 items-center">
                    <label class="sr-only" for="catalog-app">按应用筛选</label
                    ><UiSelect
                        id="catalog-app"
                        class="!w-60 max-w-full"
                        :model-value="query.application_id ?? null"
                        @update:model-value="(value) => list.setFilters({ application_id: Number(value) || undefined })"
                        ><option :value="null">全部应用</option>
                        <option v-for="app in applications.items.value" :key="app.id" :value="app.id">
                            {{ app.name }}
                        </option></UiSelect
                    ><UiButton
                        v-if="applications.hasMore.value"
                        size="sm"
                        variant="outline"
                        :disabled="applications.loading.value"
                        @click="applications.loadNext"
                        >加载更多应用</UiButton
                    >
                    <p v-if="applications.error.value" role="alert" class="text-destructive text-xs">
                        {{ applications.error.value }}
                        <button type="button" @click="applications.retry">重试</button>
                    </p>
                </div>
                <span class="text-xs text-muted-foreground">当前页 {{ rows.length }} 条权限</span
                ><UiButton class="sm:ml-auto" variant="ghost" :disabled="refreshing" @click="list.refresh"
                    ><RefreshCw :size="15" aria-hidden="true" />刷新</UiButton
                >
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!rows.length"
                @retry="list.refresh"
                ><div class="table-scroll">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th scope="col">权限名称</th>
                                <template v-if="kind === 'authority'"
                                    ><th scope="col">权限标识</th>
                                    <th scope="col">创建时间</th></template
                                ><template v-else
                                    ><th scope="col">动作</th>
                                    <th scope="col">资源类型</th>
                                    <th scope="col">资源匹配</th>
                                    <th scope="col">所属应用</th>
                                    <th scope="col">操作</th></template
                                >
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="row in rows" :key="row.id">
                                <td>
                                    <p class="font-medium">{{ row.name }}</p>
                                    <p class="text-xs text-muted-foreground mt-1">#{{ row.id }}</p>
                                </td>
                                <template v-if="'authority_uri' in row"
                                    ><td>
                                        <code class="text-xs text-primary">{{ row.authority_uri }}</code>
                                    </td>
                                    <td class="text-xs text-muted-foreground">
                                        {{ row.create_time }}
                                    </td></template
                                ><template v-else
                                    ><td>{{ row.action }}</td>
                                    <td class="text-muted-foreground">
                                        {{ row.resource_type || '*' }}
                                    </td>
                                    <td>
                                        <code class="text-xs">{{ row.resource_pattern || '*' }}</code>
                                    </td>
                                    <td>
                                        {{
                                            applications.items.value.find((app) => app.id === row.application_id)
                                                ?.name || 'App #' + row.application_id
                                        }}
                                    </td>
                                    <td>
                                        <div class="table-actions">
                                            <UiButton variant="outline" size="sm" @click="edit(row)">编辑</UiButton
                                            ><UiButton
                                                variant="ghost"
                                                size="sm"
                                                class="text-destructive"
                                                @click="remove(row)"
                                                >删除</UiButton
                                            >
                                        </div>
                                    </td></template
                                >
                            </tr>
                        </tbody>
                    </table>
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
            :title="editing === null ? '新增权限' : '编辑资源权限'"
            :busy="pending"
            :error="writeError"
            :before-close="draft.confirmDiscard"
            @confirm="save"
            ><form class="space-y-4" @submit.prevent="save">
                <template v-if="kind === 'authority'"
                    ><div class="form-field">
                        <label for="authority-name">权限名称</label
                        ><UiInput
                            id="authority-name"
                            :aria-invalid="invalidField === 'authority-name'"
                            :aria-describedby="invalidField === 'authority-name' ? 'catalog-validation' : undefined"
                            v-model="form.name"
                            :disabled="pending"
                            required
                        />
                    </div>
                    <div class="form-field">
                        <label for="authority-uri">权限标识</label
                        ><UiInput
                            id="authority-uri"
                            :aria-invalid="invalidField === 'authority-uri'"
                            :aria-describedby="invalidField === 'authority-uri' ? 'catalog-validation' : undefined"
                            v-model="form.authority_uri"
                            :disabled="pending"
                            placeholder="/users"
                            required
                        />
                    </div>
                    <div class="form-field">
                        <label for="authority-method">请求方法</label
                        ><UiSelect id="authority-method" v-model="form.method" :disabled="pending"
                            ><option
                                v-for="method in ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']"
                                :key="method"
                            >
                                {{ method }}
                            </option></UiSelect
                        >
                    </div></template
                ><template v-else
                    ><div class="form-field">
                        <label for="permission-app">所属应用</label
                        ><UiSelect
                            id="permission-app"
                            :aria-invalid="invalidField === 'permission-app'"
                            :aria-describedby="invalidField === 'permission-app' ? 'catalog-validation' : undefined"
                            v-model.number="form.application_id"
                            :disabled="pending"
                            ><option
                                v-if="!applications.items.value.some((app) => app.id === form.application_id)"
                                :value="form.application_id"
                            >
                                {{ form.application_id ? 'App #' + form.application_id : '请选择应用' }}
                            </option>
                            <option v-for="app in applications.items.value" :key="app.id" :value="app.id">
                                {{ app.name }}
                            </option></UiSelect
                        ><UiButton
                            v-if="applications.hasMore.value"
                            variant="ghost"
                            :disabled="pending || applications.loading.value"
                            @click="applications.loadNext"
                            >加载更多应用</UiButton
                        >
                    </div>
                    <div v-for="field in fields" :key="field.key" class="form-field">
                        <label :for="'permission-' + field.key">{{ field.label }}</label
                        ><UiInput
                            :id="'permission-' + field.key"
                            :aria-invalid="invalidField === 'permission-' + field.key"
                            :aria-describedby="
                                invalidField === 'permission-' + field.key ? 'catalog-validation' : undefined
                            "
                            v-model="form[field.key]"
                            :placeholder="field.placeholder"
                            :disabled="pending"
                        /></div
                ></template>
                <p v-if="validation" id="catalog-validation" class="inline-error" role="alert">{{ validation }}</p>
            </form></UiDialog
        >
    </section>
</template>
