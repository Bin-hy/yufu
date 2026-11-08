<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ShieldCheck } from '@lucide/vue'
import * as roleApi from '@/pages/roles/api'
import * as authApi from '@/pages/authority/api'
import * as permApi from '@/pages/permissions/api'
import { usePageQuery } from '@/composables/use-page-query'
import { usePagedOptions, readAllPages } from '@/composables/use-paged-options'
import { useAsyncQuery } from '@/composables/use-async-query'
import { usePermissionAssignment } from '@/composables/use-permission-assignment'
import { useFormDraft } from '@/composables/use-form-draft'
import { useTabsStore } from '@/pinia/stores/tabs'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import PageHeader from './page-header.vue'
import DataState from './data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
const props = defineProps<{ kind: 'authority' | 'permission' }>(),
    pager = usePageQuery(),
    tabs = useTabsStore()
const roleId = computed(() => pager.query.value.roleId ?? 0),
    search = ref('')
const roles = usePagedOptions(
    (p, s, signal) => roleApi.getRolesByPage(p, s, { signal, errorPresentation: 'local' }),
    () => String(tabs.invalidations[String(pager.context.route.value.name)] ?? 0),
)
const read = (signal: AbortSignal) =>
    props.kind === 'authority'
        ? readAllPages(
              (p, s) =>
                  authApi.getAuthorityByRoleId(roleId.value, p, s, {
                      signal,
                      errorPresentation: 'local',
                  }),
              signal,
          ).then((rows) => rows.map((row) => row.id))
        : readAllPages(
              (p, s) =>
                  permApi.getRolePermissionsByRoleId(roleId.value, p, s, {
                      signal,
                      errorPresentation: 'local',
                  }),
              signal,
          ).then((rows) => rows.map((row) => row.id))
const assignment = usePermissionAssignment({
    read,
    add: (ids) =>
        props.kind === 'authority'
            ? authApi.addRoleAuthBatch(
                  { role_id: roleId.value, authority_ids: ids },
                  { errorPresentation: 'local' },
              )
            : permApi.addRolePermissionBatch(
                  { role_id: roleId.value, permission_ids: ids },
                  { errorPresentation: 'local' },
              ),
    remove: (ids) =>
        props.kind === 'authority'
            ? authApi.deleteRoleAuthBatch(
                  { role_id: roleId.value, authority_ids: ids },
                  { errorPresentation: 'local' },
              )
            : permApi.deleteRolePermissionBatch(
                  { role_id: roleId.value, permission_ids: ids },
                  { errorPresentation: 'local' },
              ),
})
const { desired, actual, ready, pending, error, dirty } = assignment
const baseline = useAsyncQuery({
    key: () => String(roleId.value),
    active: () => pager.context.active.value && roleId.value > 0,
    loader: read,
})
const catalog = usePagedOptions(
    async (p, size, signal) => {
        const options = { signal, errorPresentation: 'local' as const }
        if (props.kind === 'authority') {
            const result = await authApi.getAuthoritiesByPage(p, size, options)
            return {
                ...result,
                data_list: result.data_list.map((row) => ({
                    id: row.id,
                    name: row.name,
                    description: row.authority_uri,
                })),
            }
        }
        const result = await permApi.getPermissionsByPage(p, size, null, options)
        return {
            ...result,
            data_list: result.data_list.map((row) => ({
                id: row.id,
                name: row.name,
                description:
                    row.action +
                    ' · ' +
                    (row.resource_type ?? '*') +
                    ':' +
                    (row.resource_pattern ?? '*'),
            })),
        }
    },
    () => props.kind + ':' + (tabs.invalidations[String(pager.context.route.value.name)] ?? 0),
)
const catalogStatus = computed(() =>
    catalog.items.value.length
        ? 'ready'
        : catalog.loading.value
          ? 'loading'
          : catalog.error.value
            ? 'error'
            : 'ready',
)
async function refreshCatalog() {
    catalog.reset()
    await catalog.loadNext()
}
const draft = useFormDraft('角色权限分配', () => [...desired.value].sort((a, b) => a - b), {
    enabled: () => roleId.value > 0,
    submitting: pending,
    discard: () => {
        desired.value = [...actual.value]
    },
})
watch(baseline.data, (ids) => {
    if (ids) {
        assignment.initialize(ids, dirty.value)
        draft.setBaseline([...actual.value].sort((a, b) => a - b))
    }
})
watch(
    () => tabs.invalidations[String(pager.context.route.value.name)] ?? 0,
    () => {
        baseline.invalidate()
    },
)
watch(
    roleId,
    () => {
        ready.value = false
        desired.value = []
        actual.value = []
        draft.markSaved()
    },
    { flush: 'sync' },
)
const rows = computed(
    () =>
        catalog.items.value.filter((row) =>
            (row.name + ' ' + row.description).toLowerCase().includes(search.value.toLowerCase()),
        ) ?? [],
)
async function select(value: unknown) {
    if (pending.value) return
    if (
        dirty.value &&
        !(await showModal({
            title: '放弃当前角色的修改？',
            content: '切换角色后，当前未保存的权限选择会丢失。',
            destructive: true,
            confirmText: '放弃并切换',
        }))
    )
        return
    await pager.setFilters({ roleId: Number(value) || undefined })
}
function toggle(id: number, value: boolean) {
    if (!ready.value || pending.value) return
    desired.value = value
        ? [...new Set([...desired.value, id])]
        : desired.value.filter((item) => item !== id)
}
async function save() {
    await assignment.save()
    if (ready.value) {
        draft.setBaseline([...actual.value].sort((a, b) => a - b))
        tabs.invalidate('user-list')
    }
    if (ready.value && !dirty.value && !error.value) {
        draft.markSaved()
        emitToast({ type: 'success', message: '角色权限已保存' })
    }
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader
            :title="kind === 'authority' ? 'API 权限分配' : '资源权限分配'"
            description="选择角色，检查已有授权后保存所需变更。"
            eyebrow="ACCESS CONTROL"
            ><template #actions
                ><UiButton :disabled="!ready || !dirty || pending" @click="save"
                    ><ShieldCheck :size="16" aria-hidden="true" />{{
                        pending ? '正在保存…' : '保存权限'
                    }}</UiButton
                ></template
            ></PageHeader
        >
        <div class="panel">
            <div class="query-toolbar">
                <div class="form-field w-64 max-w-full">
                    <label :for="kind + '-role'">选择角色</label
                    ><UiSelect
                        :id="kind + '-role'"
                        :model-value="roleId"
                        :disabled="pending || roles.loading.value"
                        @update:model-value="select"
                        ><option :value="0">请选择角色</option>
                        <option v-for="role in roles.items.value" :key="role.id" :value="role.id">
                            {{ role.name }}
                        </option></UiSelect
                    >
                </div>
                <UiButton
                    v-if="roles.hasMore.value"
                    variant="outline"
                    :disabled="roles.loading.value || pending"
                    @click="roles.loadNext"
                    >加载更多角色</UiButton
                >
                <p v-if="roles.error.value" class="inline-error" role="alert">
                    {{ roles.error.value }} <button type="button" @click="roles.retry">重试</button>
                </p>
                <UiInput
                    v-model="search"
                    class="search-field sm:ml-auto"
                    aria-label="搜索权限"
                    placeholder="搜索已加载的权限…"
                />
            </div>
            <p v-if="!roleId" class="data-state">选择一个角色开始分配权限。</p>
            <template v-else
                ><p v-if="error" class="inline-error m-4" role="alert">
                    {{ error }}
                    <button
                        v-if="!ready"
                        type="button"
                        class="underline"
                        @click="assignment.reconcile"
                    >
                        重新核对授权
                    </button>
                </p>
                <DataState
                    :status="baseline.status.value"
                    :error="baseline.error.value"
                    @retry="baseline.refresh"
                    ><DataState
                        :status="catalogStatus"
                        :error="catalog.error.value"
                        :empty="!rows.length"
                        @retry="refreshCatalog"
                        ><div class="panel-heading text-xs text-muted-foreground">
                            <span
                                >已选 {{ desired.length }} 项 · 已授权 {{ actual.length }} 项 ·
                                已加载 {{ catalog.items.value.length }} 项候选</span
                            ><span v-if="dirty" class="text-primary">有未保存的修改</span>
                        </div>
                        <div class="divide-y divide-border">
                            <label
                                v-for="row in rows"
                                :key="row.id"
                                class="flex gap-3 items-center px-5 py-4 hover:bg-muted/50"
                                ><UiCheckbox
                                    :model-value="desired.includes(row.id)"
                                    :disabled="!ready || pending"
                                    @update:model-value="(value) => toggle(row.id, value)"
                                /><span class="min-w-0"
                                    ><span class="block font-medium">{{ row.name }}</span
                                    ><span
                                        class="block text-xs text-muted-foreground mt-1 break-all"
                                        >{{ row.description }}</span
                                    ></span
                                ></label
                            >
                        </div></DataState
                    ></DataState
                >
                <div v-if="catalog.hasMore.value" class="p-4 border-t border-border">
                    <UiButton
                        variant="outline"
                        :disabled="pending || catalog.loading.value"
                        @click="catalog.loadNext"
                        >{{ catalog.loading.value ? '正在加载…' : '加载更多权限' }}</UiButton
                    >
                    <span class="ml-3 text-xs text-muted-foreground"
                        >搜索仅覆盖已加载的候选；已授权项保持完整。</span
                    >
                </div>
            </template>
        </div>
    </section>
</template>
