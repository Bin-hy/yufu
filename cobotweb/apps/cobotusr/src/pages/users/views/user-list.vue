<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useTabsStore } from '@/pinia/stores/tabs'
import { Plus, RefreshCw, Users } from '@lucide/vue'
import type { UserVO } from '@/types/models'
import * as api from '../api'
import { kickoutUserRequest } from '@/pages/login/api'
import { usePagedList } from '@/composables/use-paged-list'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import { readAllPages } from '@/composables/use-paged-options'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import ApiPanel from '../components/user-api-authority-panel.vue'
import ResourcePanel from '../components/user-resource-permission-panel.vue'
const list = usePagedList((q, signal) =>
    api.getUsersByPage(q.page_number, q.page_size, { signal, errorPresentation: 'local' }),
)
const { page, size, search, status, error, refreshing, totalPages } = list
const roles = useAsyncQuery({
    key: 'modifiable-roles',
    active: list.context.active,
    loader: (signal) =>
        readAllPages((p, s) => api.getModifiableRoles(p, s, { signal, errorPresentation: 'local' }), signal),
})
const tabs = useTabsStore()
watch(
    () => tabs.invalidations['user-list'] ?? 0,
    () => roles.invalidate(),
)
const roleItems = computed(() => roles.data.value ?? []),
    roleError = roles.error
const removed = ref(new Set<number>())
const rows = computed(() =>
    list.rows.value.filter(
        (user) =>
            !removed.value.has(user.id) &&
            (user.username + ' ' + (user.nickname ?? '') + ' ' + (user.role?.name ?? ''))
                .toLowerCase()
                .includes(search.value.toLowerCase()),
    ),
)
const pendingRole = ref<Record<number, number>>({}),
    mutation = useAsyncMutation()
const { pending, error: writeError } = mutation
useFormDraft('用户角色选择', () => pendingRole.value, {
    submitting: pending,
    discard: () => {
        pendingRole.value = {}
    },
})
function select(user: UserVO, value: unknown) {
    const next = Number(value)
    if (!roleItems.value.some((role) => role.id === next)) return
    if (next === user.role_id) delete pendingRole.value[user.id]
    else pendingRole.value[user.id] = next
}
async function save(user: UserVO) {
    const id = pendingRole.value[user.id]
    if (id === undefined || id === user.role_id) return
    await mutation.run(
        () => api.patchUserRole({ user_id: user.id, new_role_id: id }, { errorPresentation: 'local' }),
        async (saved) => {
            if (saved?.id === user.id) Object.assign(user, saved)
            delete pendingRole.value[user.id]
            emitToast({ type: 'success', message: '角色更新成功' })
            await list.refresh()
        },
    )
}
function remove(user: UserVO) {
    void showModal({
        title: '删除用户？',
        content: '用户「' + user.username + '」将被删除，此操作无法撤销。',
        destructive: true,
        confirmText: '删除用户',
        confirmCallback: async () => {
            await api.deleteUser(user.id, { errorPresentation: 'local' })
            removed.value.add(user.id)
            delete pendingRole.value[user.id]
            emitToast({ type: 'success', message: '用户已删除' })
            await list.refresh()
        },
    })
}
function kickout(user: UserVO) {
    void showModal({
        title: '将用户踢下线？',
        content: '「' + user.username + '」的当前登录会话将失效。',
        destructive: true,
        confirmText: '踢下线',
        confirmCallback: async () => {
            await kickoutUserRequest({ sub: user.id }, { errorPresentation: 'local' })
            emitToast({ type: 'success', message: '用户已下线' })
        },
    })
}
const overrideOpen = ref(false),
    overrideUser = ref<UserVO | null>(null),
    overrideTab = ref<'authority' | 'permission'>('authority'),
    overrideBusy = ref(false)
function openOverride(user: UserVO) {
    overrideUser.value = user
    overrideTab.value = 'authority'
    overrideOpen.value = true
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader title="用户管理" description="管理平台账户、角色与个人权限。" eyebrow="IDENTITY"
            ><template #actions
                ><UiButton variant="outline" :disabled="status === 'loading' || refreshing" @click="list.refresh"
                    ><RefreshCw :size="15" aria-hidden="true" />刷新</UiButton
                ><RouterLink :to="{ name: 'create-user' }" class="ui-button ui-button-default"
                    ><Plus :size="16" aria-hidden="true" />创建用户</RouterLink
                ></template
            ></PageHeader
        >
        <div class="panel">
            <div class="query-toolbar">
                <UiInput
                    v-model="search"
                    class="search-field"
                    aria-label="筛选当前页用户"
                    placeholder="搜索当前页的用户名、昵称或角色…"
                /><span class="text-xs text-muted-foreground flex items-center gap-2"
                    ><Users :size="14" aria-hidden="true" />当前页 {{ list.rows.value.length }} 个账户</span
                >
            </div>
            <p v-if="roleError" class="inline-error m-4" role="alert">
                角色候选加载失败：{{ roleError }}
                <UiButton variant="ghost" size="sm" @click="roles.refresh">重试</UiButton>
            </p>
            <p v-if="writeError" class="inline-error m-4" role="alert">{{ writeError }}</p>
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
                                <th scope="col">账户</th>
                                <th scope="col">角色</th>
                                <th scope="col">创建时间</th>
                                <th scope="col">操作</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="user in rows" :key="user.id">
                                <td>
                                    <p class="font-medium">{{ user.nickname || user.username }}</p>
                                    <p class="text-xs text-muted-foreground mt-1">
                                        {{ user.username }} · #{{ user.id }}
                                    </p>
                                </td>
                                <td>
                                    <div class="flex items-center gap-2">
                                        <UiSelect
                                            class="!w-36"
                                            :aria-label="user.username + '的角色'"
                                            :model-value="pendingRole[user.id] ?? user.role_id"
                                            :disabled="pending || roles.status.value !== 'ready'"
                                            @update:model-value="(value) => select(user, value)"
                                            ><option
                                                v-if="!roleItems.some((role) => role.id === user.role_id)"
                                                :value="user.role_id"
                                            >
                                                {{ user.role?.name || '当前角色' }}
                                            </option>
                                            <option v-for="role in roleItems" :key="role.id" :value="role.id">
                                                {{ role.name }}
                                            </option></UiSelect
                                        ><UiButton
                                            v-if="pendingRole[user.id] !== undefined"
                                            size="sm"
                                            :disabled="pending"
                                            @click="save(user)"
                                            >保存</UiButton
                                        >
                                    </div>
                                </td>
                                <td class="text-muted-foreground text-xs">
                                    {{ user.create_time || '—' }}
                                </td>
                                <td>
                                    <div class="table-actions">
                                        <UiButton
                                            variant="ghost"
                                            size="sm"
                                            :disabled="pending"
                                            @click="openOverride(user)"
                                            >权限覆写</UiButton
                                        ><UiButton variant="ghost" size="sm" :disabled="pending" @click="kickout(user)"
                                            >踢下线</UiButton
                                        ><UiButton
                                            variant="ghost"
                                            size="sm"
                                            class="text-destructive"
                                            :disabled="pending"
                                            @click="remove(user)"
                                            >删除</UiButton
                                        >
                                    </div>
                                </td>
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
            v-model:open="overrideOpen"
            :title="'权限覆写 · ' + (overrideUser?.username ?? '')"
            description="单独控制此用户的权限，优先于角色默认授权。"
            size="xl"
            hide-footer
            :busy="overrideBusy"
            ><template v-if="overrideUser"
                ><div class="flex gap-2 mb-5" role="group" aria-label="权限类型">
                    <UiButton
                        :variant="overrideTab === 'authority' ? 'secondary' : 'ghost'"
                        :aria-pressed="overrideTab === 'authority'"
                        :disabled="overrideBusy"
                        @click="overrideTab = 'authority'"
                        >API 权限</UiButton
                    ><UiButton
                        :variant="overrideTab === 'permission' ? 'secondary' : 'ghost'"
                        :aria-pressed="overrideTab === 'permission'"
                        :disabled="overrideBusy"
                        @click="overrideTab = 'permission'"
                        >资源权限</UiButton
                    >
                </div>
                <ApiPanel
                    v-if="overrideOpen"
                    v-show="overrideTab === 'authority'"
                    :key="overrideUser.id + 'a'"
                    :user="overrideUser"
                    :active="overrideTab === 'authority'"
                    @busy="overrideBusy = $event" /><ResourcePanel
                    v-if="overrideOpen"
                    v-show="overrideTab === 'permission'"
                    :key="overrideUser.id + 'p'"
                    :user="overrideUser"
                    :active="overrideTab === 'permission'"
                    @busy="overrideBusy = $event" /></template
        ></UiDialog>
    </section>
</template>
