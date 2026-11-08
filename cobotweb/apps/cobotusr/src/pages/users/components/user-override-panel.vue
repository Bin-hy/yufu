<script setup lang="ts">
import { computed, watch } from 'vue'
import type { UserVO } from '@/types/models'
import * as userApi from '../api'
import * as authorityApi from '@/pages/authority/api'
import * as permissionApi from '@/pages/permissions/api'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { readAllPages } from '@/composables/use-paged-options'
import { useFormDraft } from '@/composables/use-form-draft'
import { usePageContext } from '@/workspace/page-context'
import UiSelect from '@/components/ui/select/select.vue'
import UiInput from '@/components/ui/input/input.vue'
import DataState from '@/components/workspace/data-state.vue'
import { ref } from 'vue'
const props = defineProps<{ user: UserVO; kind: 'authority' | 'permission'; active: boolean }>()
const emit = defineEmits<{ busy: [value: boolean] }>(),
    context = usePageContext(),
    query = ref('')
const mutation = useAsyncMutation(),
    { pending, error } = mutation
watch(pending, (value) => emit('busy', value))
useFormDraft('用户权限覆写', () => null, { submitting: pending })
const state = useAsyncQuery({
    key: () => props.kind + ':' + props.user.id,
    active: () => props.active && context.active.value,
    loader: async (signal) => {
        const options = { signal, errorPresentation: 'local' as const },
            id = props.user.role_id
        if (props.kind === 'authority') {
            const [all, role, overrides] = await Promise.all([
                readAllPages((p, s) => authorityApi.getAuthoritiesByPage(p, s, options), signal),
                readAllPages(
                    (p, s) => authorityApi.getAuthorityByRoleId(id, p, s, options),
                    signal,
                ),
                readAllPages(
                    (p, s) => userApi.getUserAuthorityOverridesByPage(p, s, options),
                    signal,
                ),
            ])
            return all.map((item) => ({
                id: item.id,
                name: item.name,
                description: item.authority_uri,
                role: role.some((r) => r.id === item.id),
                override: overrides.find(
                    (r) => r.user_vo?.id === props.user.id && r.authority_vo?.id === item.id,
                )?.allow,
            }))
        }
        const [all, role, overrides] = await Promise.all([
            readAllPages((p, s) => permissionApi.getPermissionsByPage(p, s, null, options), signal),
            readAllPages(
                (p, s) => permissionApi.getRolePermissionsByRoleId(id, p, s, options),
                signal,
            ),
            readAllPages((p, s) => userApi.getUserPermissionOverridesByPage(p, s, options), signal),
        ])
        return all.map((item) => ({
            id: item.id,
            name: item.name,
            description: item.action,
            role: role.some((r) => r.id === item.id),
            override: overrides.find(
                (r) => r.user_id === props.user.id && r.permission_id === item.id,
            )?.allow,
        }))
    },
})
const { status, error: readError, refreshing } = state
const rows = computed(
    () =>
        state.data.value?.filter((row) =>
            (row.name + ' ' + row.description).toLowerCase().includes(query.value.toLowerCase()),
        ) ?? [],
)
async function change(id: number, value: unknown) {
    const row = state.data.value?.find((item) => item.id === id)
    if (!row || pending.value) return
    const allow = value === 'allow',
        options = { errorPresentation: 'local' as const }
    await mutation.run(
        async () => {
            if (props.kind === 'authority') {
                const payload = {
                    user_id: props.user.id,
                    authority_id: id,
                    allow: row.override ?? allow,
                }
                if (value === 'inherit') {
                    if (row.override !== undefined)
                        await userApi.deleteUserAuthorityOverride(payload, options)
                } else
                    await (
                        row.override === undefined
                            ? userApi.addUserAuthorityOverride
                            : userApi.patchUserAuthorityOverride
                    )({ ...payload, allow }, options)
            } else {
                const payload = { user_id: props.user.id, permission_id: id, allow }
                if (value === 'inherit') {
                    if (row.override !== undefined)
                        await userApi.deleteUserPermissionOverride(payload, options)
                } else
                    await (
                        row.override === undefined
                            ? userApi.addUserPermissionOverride
                            : userApi.patchUserPermissionOverride
                    )(payload, options)
            }
        },
        async () => {
            row.override = value === 'inherit' ? undefined : allow
            await state.refresh()
        },
    )
}
</script>
<template>
    <div class="space-y-4">
        <p class="form-help">继承角色权限；允许或拒绝会立即保存为用户覆写。</p>
        <UiInput v-model="query" aria-label="搜索权限" placeholder="搜索权限名称或标识…" />
        <p v-if="error" class="inline-error" role="alert">{{ error }}</p>
        <DataState
            :status="status"
            :error="readError"
            :refreshing="refreshing"
            :empty="!rows.length"
            @retry="state.refresh"
            ><div class="table-scroll max-h-[440px]">
                <table class="data-table">
                    <thead>
                        <tr>
                            <th scope="col">权限</th>
                            <th scope="col">角色默认</th>
                            <th scope="col">用户覆写</th>
                        </tr>
                    </thead>
                    <tbody>
                        <tr v-for="row in rows" :key="row.id">
                            <td>
                                <p class="font-medium">{{ row.name }}</p>
                                <p class="text-xs text-muted-foreground mt-1">
                                    {{ row.description }}
                                </p>
                            </td>
                            <td>{{ row.role ? '允许' : '未授权' }}</td>
                            <td>
                                <UiSelect
                                    :model-value="
                                        row.override === undefined
                                            ? 'inherit'
                                            : row.override
                                              ? 'allow'
                                              : 'deny'
                                    "
                                    :aria-label="row.name + '的覆写'"
                                    :disabled="pending"
                                    @update:model-value="(value) => change(row.id, value)"
                                    ><option value="inherit">继承角色</option>
                                    <option value="allow">允许</option>
                                    <option value="deny">拒绝</option></UiSelect
                                >
                            </td>
                        </tr>
                    </tbody>
                </table>
            </div></DataState
        >
    </div>
</template>
