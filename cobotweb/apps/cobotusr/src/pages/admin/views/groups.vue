<script setup lang="ts">
defineOptions({ name: 'AdminGroups' })
import { onBeforeUnmount, reactive, ref, watch } from 'vue'
import { Network, Pencil, Plus, Search, Trash2, UserPlus, Users } from '@lucide/vue'
import { listGroups, groupChange, searchAccounts } from '../api'
import type { Group } from '../api/type'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useOperationKey } from '@/composables/use-ephemeral-result'
import { emitToast } from '@/utils/toast'
import { showModal } from '@/utils/modal'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import EntityPicker from '@/components/ui/entity-picker/entity-picker.vue'
import type { PickerOption } from '@/components/ui/entity-picker/types'
import { accountOption } from '@/lib/picker-options'
const directoryQuery = ref(''),
    debouncedDirectoryQuery = ref('')
let directoryTimer: number | undefined
watch(directoryQuery, (value) => {
    window.clearTimeout(directoryTimer)
    directoryTimer = window.setTimeout(() => {
        debouncedDirectoryQuery.value = value.trim()
    }, 240)
})
onBeforeUnmount(() => window.clearTimeout(directoryTimer))
const list = useCursorPage(
    () => `enterprise-groups:${debouncedDirectoryQuery.value}`,
    (after, signal) =>
        listGroups(after, signal, debouncedDirectoryQuery.value || undefined),
),
    { data, status, error, refreshing } = list
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation,
    key = useOperationKey()
const form = reactive({ name: '', description: '' }),
    selected = ref<Group>(),
    member = ref<number | ''>(''),
    memberLabel = ref(''),
    memberSecondary = ref(''),
    editorOpen = ref(false)
watch(debouncedDirectoryQuery, () => list.first())
const loadAccountOptions = async (query: string, signal: AbortSignal): Promise<PickerOption[]> => {
    const page = await searchAccounts(query, signal)
    return page.items.map(accountOption)
}
function chooseMember(option: PickerOption) {
    member.value = Number(option.value)
    memberLabel.value = option.label
    memberSecondary.value = option.secondary ?? ''
}
async function change(path: string, method: 'POST' | 'PATCH' | 'DELETE', body: unknown) {
    await mutation.run(
        () => groupChange(path, method, body, key({ path, body })),
        async () => {
            selected.value = undefined
            await list.refresh()
            emitToast({ type: 'success', message: method === 'DELETE' ? '用户组已删除' : '用户组已更新' })
        },
    )
}
function create() {
    selected.value = undefined
    form.name = ''
    form.description = ''
    editorOpen.value = true
}
function edit(group: Group) {
    selected.value = group
    form.name = group.name
    form.description = group.description ?? ''
    editorOpen.value = true
}
async function save() {
    if (!form.name.trim()) {
        emitToast({ type: 'warning', message: '请输入用户组名称' })
        document.getElementById('group-name')?.focus()
        return
    }
    await change(
        selected.value ? '/' + selected.value.group_id : '',
        selected.value ? 'PATCH' : 'POST',
        {
            name: form.name.trim(),
            description: form.description.trim() || null,
            ...(selected.value ? { expected_revision: selected.value.revision } : {}),
        },
    )
    if (!writeError.value) {
        editorOpen.value = false
        form.name = ''
        form.description = ''
    }
}
async function remove(group: Group) {
    const confirmed = await showModal({
        title: '删除用户组',
        content: `删除“${group.name}”会移除该组的成员关系与访问来源。组内成员账户不会被删除。`,
        confirmText: '删除组并移除访问来源',
        destructive: true,
    })
    if (confirmed)
        await change('/' + group.group_id, 'DELETE', { expected_revision: group.revision })
}
async function addMember(group: Group) {
    if (!member.value) return
    await change(`/${group.group_id}/members/${Number(member.value)}`, 'PATCH', {
        present: true,
        expected_revision: group.revision,
    })
    member.value = ''
    memberLabel.value = ''
    memberSecondary.value = ''
}
async function removeMember(group: Group, userId: number) {
    await change(`/${group.group_id}/members/${userId}`, 'PATCH', {
        present: false,
        expected_revision: group.revision,
    })
}
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="用户组"
            eyebrow="GROUPS"
            description="扁平用户组仅汇总已明确分配的应用访问。组成员关系不会自动扩大到其他应用。"
        >
            <UiButton @click="create"><Plus :size="14" aria-hidden="true" />新建用户组</UiButton>
        </PageHeader>
        <p v-if="writeError" role="alert" class="form-alert mb-4">{{ writeError }}</p>

        <div class="table-toolbar mb-4 rounded-xl border border-border">
            <div class="search-box">
                <Search :size="15" aria-hidden="true" />
                <UiInput
                    v-model="directoryQuery"
                    clearable
                    placeholder="搜索用户组名称或 ID…"
                    aria-label="搜索用户组"
                />
            </div>
            <span class="toolbar-spacer text-[11px] text-muted-foreground">
                {{ data?.items.length ?? 0 }} 个用户组
            </span>
        </div>

        <DataState
            :status="status"
            :error="error"
            :refreshing="refreshing"
            :empty="!data?.items.length"
            empty-title="还没有用户组"
            empty-description="创建第一个用户组，把成员和访问来源集中管理。"
            :skeleton-rows="3"
            @retry="list.refresh()"
        >
            <div class="grid gap-4 md:grid-cols-2">
                <article v-for="group in data?.items" :key="group.group_id" class="panel">
                    <header class="panel-heading">
                        <div class="flex min-w-0 items-start gap-3">
                            <span class="list-icon !h-9 !w-9"><Network :size="16" aria-hidden="true" /></span>
                            <div class="min-w-0">
                                <h2 class="truncate">{{ group.name }}</h2>
                                <p class="mono truncate text-[10.5px]" :title="group.group_id">组 ID · {{ group.group_id.slice(0, 8) }}…</p>
                            </div>
                        </div>
                        <div class="flex items-center gap-1">
                            <button type="button" class="icon-button" aria-label="编辑用户组" title="编辑" @click="edit(group)">
                                <Pencil :size="14" aria-hidden="true" />
                            </button>
                            <button type="button" class="icon-button text-destructive" aria-label="删除用户组" title="删除" @click="remove(group)">
                                <Trash2 :size="14" aria-hidden="true" />
                            </button>
                        </div>
                    </header>
                    <div class="surface-body space-y-4">
                        <p class="text-xs text-muted-foreground">
                            {{ group.description || '未填写说明' }}
                        </p>
                        <div>
                            <p class="mb-2 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
                                已分配应用 · {{ group.application_ids.length }}
                            </p>
                            <div class="flex flex-wrap gap-1.5">
                                <span v-for="appId in group.application_ids" :key="appId" class="status-badge status-brand">应用 {{ appId }}</span>
                                <span v-if="!group.application_ids.length" class="text-[11px] text-muted-foreground">尚未分配应用访问</span>
                            </div>
                        </div>
                        <div>
                            <p class="mb-2 flex items-center gap-1.5 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
                                <Users :size="11" aria-hidden="true" />成员 · {{ group.members.length }}
                            </p>
                            <div class="flex flex-wrap gap-1.5">
                                <button
                                    v-for="profile in group.member_profiles || group.members.map(id => ({ user_id: id, username: '', nickname: '' }))"
                                    :key="profile.user_id"
                                    type="button"
                                    class="group inline-flex max-w-full items-center gap-1.5 rounded-md border border-border bg-muted/40 px-2 py-1 text-[10.5px] transition-colors hover:border-destructive/30 hover:text-destructive"
                                    :disabled="pending"
                                    :aria-label="'移除账户 ' + (profile.nickname || profile.username || profile.user_id)"
                                    @click="removeMember(group, profile.user_id)"
                                >
                                    <span class="truncate">{{ profile.nickname || profile.username || '账户 ' + profile.user_id }}</span>
                                    <span v-if="profile.username" class="truncate opacity-60">@{{ profile.username }}</span>
                                    <span class="opacity-50 group-hover:opacity-100">×</span>
                                </button>
                                <span v-if="!group.members.length" class="text-[11px] text-muted-foreground">暂无成员</span>
                            </div>
                        </div>
                        <form class="flex items-end gap-2 border-t border-border pt-4" @submit.prevent="addMember(group)">
                            <UiFormField :id="'group-member-' + group.group_id" label="搜索并添加成员" class="min-w-0 flex-1" hint="按显示名称、用户名或账户 ID 搜索。">
                                <EntityPicker
                                    :id="'group-member-' + group.group_id"
                                    :model-value="member"
                                    :selected-label="memberLabel"
                                    :selected-secondary="memberSecondary"
                                    :load="loadAccountOptions"
                                    placeholder="搜索账户名称或用户名…"
                                    :disabled="pending"
                                    @update:model-value="(value) => { if (value === null) { member = ''; memberLabel = ''; memberSecondary = '' } }"
                                    @select="chooseMember"
                                />
                            </UiFormField>
                            <UiButton type="submit" variant="outline" size="sm" :disabled="pending || !member">
                                <UserPlus :size="13" aria-hidden="true" />添加成员
                            </UiButton>
                        </form>
                    </div>
                </article>
            </div>
            <div v-if="data?.next" class="mt-4 flex justify-center">
                <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="list.next()">加载更多用户组</UiButton>
            </div>
        </DataState>

        <UiDialog
            v-model:open="editorOpen"
            :title="selected ? '编辑用户组' : '新建用户组'"
            description="用户组只汇总明确分配的应用访问，不会改变全局权限。"
            :confirm-text="selected ? '保存用户组' : '创建用户组'"
            :busy="pending"
            :error="writeError"
            @confirm="save"
        >
            <div class="space-y-4">
                <UiFormField id="group-name" label="组名称" required>
                    <UiInput id="group-name" v-model="form.name" required :disabled="pending" placeholder="例如：数据平台协作者" />
                </UiFormField>
                <UiFormField id="group-description" label="说明" optional>
                    <UiInput id="group-description" v-model="form.description" :disabled="pending" placeholder="说明这个组的用途与边界" />
                </UiFormField>
            </div>
        </UiDialog>
    </div>
</template>
