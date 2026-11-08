<script setup lang="ts">
import { ref } from 'vue'
import { Plus, RefreshCw } from '@lucide/vue'
import * as api from '../api'
import type { RoleVO } from '@/types/models'
import { usePagedList } from '@/composables/use-paged-list'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import { useTabsStore } from '@/pinia/stores/tabs'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import UiInput from '@/components/ui/input/input.vue'
const list = usePagedList((q, signal) =>
        api.getRolesByPage(q.page_number, q.page_size, { signal, errorPresentation: 'local' }),
    ),
    { rows, page, size, totalPages, status, error, refreshing } = list
const open = ref(false),
    id = ref<number | null>(null),
    name = ref(''),
    validation = ref(''),
    mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation,
    tabs = useTabsStore()
const draft = useFormDraft('角色编辑', () => name.value.trim(), {
    enabled: open,
    submitting: pending,
    discard: () => {
        open.value = false
        name.value = ''
    },
})
function edit(role?: RoleVO) {
    id.value = role?.id ?? null
    name.value = role?.name ?? ''
    validation.value = ''
    writeError.value = null
    draft.markSaved()
    open.value = true
}
async function save() {
    validation.value = ''
    if (!name.value.trim()) {
        validation.value = '请输入角色名称'
        document.getElementById('role-name')?.focus()
        return
    }
    await mutation.run(
        () =>
            id.value === null
                ? api.addRole({ name: name.value.trim() }, { errorPresentation: 'local' })
                : api.patchRole({ id: id.value, name: name.value.trim() }, { errorPresentation: 'local' }),
        async () => {
            draft.markSaved()
            open.value = false
            tabs.invalidate('user-list')
            tabs.invalidate('create-user')
            tabs.invalidate('authority-assign')
            tabs.invalidate('permission-assign')
            emitToast({ type: 'success', message: '角色已保存' })
            await list.refresh()
        },
    )
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader title="角色管理" description="定义账户身份，为角色统一分配权限。" eyebrow="IDENTITY"
            ><template #actions
                ><UiButton variant="outline" :disabled="refreshing" @click="list.refresh"
                    ><RefreshCw :size="15" aria-hidden="true" />刷新</UiButton
                ><UiButton @click="edit()"><Plus :size="16" aria-hidden="true" />创建角色</UiButton></template
            ></PageHeader
        >
        <div class="panel">
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
                                <th scope="col">角色名称</th>
                                <th scope="col">创建时间</th>
                                <th scope="col">更新时间</th>
                                <th scope="col">操作</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="role in rows" :key="role.id">
                                <td>
                                    <p class="font-medium">{{ role.name }}</p>
                                    <p class="text-muted-foreground text-xs mt-1">#{{ role.id }}</p>
                                </td>
                                <td class="text-xs text-muted-foreground">
                                    {{ role.create_time }}
                                </td>
                                <td class="text-xs text-muted-foreground">
                                    {{ role.update_time }}
                                </td>
                                <td>
                                    <UiButton variant="outline" size="sm" @click="edit(role)">重命名</UiButton>
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
            v-model:open="open"
            :title="id === null ? '创建角色' : '重命名角色'"
            :busy="pending"
            :error="writeError"
            :before-close="draft.confirmDiscard"
            @confirm="save"
            ><form class="form-field" @submit.prevent="save">
                <label for="role-name">角色名称</label
                ><UiInput
                    id="role-name"
                    v-model="name"
                    :disabled="pending"
                    :aria-invalid="!!validation"
                    aria-describedby="role-validation"
                    autofocus
                />
                <p id="role-validation" class="text-xs text-destructive">{{ validation }}</p>
            </form></UiDialog
        >
    </section>
</template>
