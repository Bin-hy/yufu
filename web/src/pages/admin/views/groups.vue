<script setup lang="ts">
defineOptions({ name: 'AdminGroups' })
import { onBeforeUnmount, reactive, ref, watch } from 'vue'
import { Network, Pencil, Plus, Search, Trash2, UserPlus, Users } from '@lucide/vue'
import { listGroups, groupChange, searchAccounts } from '../api'
import type { Group, GroupMember } from '../api/type'
import { useCursorList } from '@/composables/use-cursor-list'
import { useRowMutation } from '@/composables/use-row-mutation'
import { useRouteFilters } from '@/composables/use-route-filters'
import { useOperationKey } from '@/composables/use-ephemeral-result'
import { emitToast } from '@/utils/toast'
import FailureFeedback from '@/components/Feedback/failure-feedback.vue'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import EntityPicker from '@/components/ui/entity-picker/entity-picker.vue'
import type { PickerOption } from '@/components/ui/entity-picker/types'
import { accountOption } from '@/lib/picker-options'

const filters = useRouteFilters({ q: '' })
const directoryQuery = ref(filters.values.value.q)
let directoryTimer: number | undefined
watch(directoryQuery, (value) => {
    window.clearTimeout(directoryTimer)
    directoryTimer = window.setTimeout(() => {
        void filters.set({ q: value.trim() })
    }, 240)
})
watch(
    () => filters.values.value.q,
    (value) => {
        if (value !== directoryQuery.value.trim()) directoryQuery.value = value
    },
)
onBeforeUnmount(() => window.clearTimeout(directoryTimer))
const list = useCursorList(
    () => `enterprise-groups:${filters.values.value.q}`,
    (after, signal) =>
        listGroups(after, signal, filters.values.value.q || undefined),
),
    { items, hasMore, status, error, refreshing } = list
const mutation = useRowMutation(),
    { pending, error: writeError, rowPending } = mutation,
    key = useOperationKey()
const form = reactive({ name: '', description: '' }),
    selected = ref<Group>(),
    editorOpen = ref(false)
/** Member-picker state is isolated per group so choosing a person in one card never leaks into another. */
interface MemberDraft { picks: PickerOption[]; pickerValue: number | null }
const drafts = reactive<Record<string, MemberDraft>>({})
function draft(group: Group): MemberDraft {
    return (drafts[group.group_id] ??= { picks: [], pickerValue: null })
}
function pickMember(group: Group, option: PickerOption) {
    const target = draft(group)
    const id = Number(option.value)
    if (
        !target.picks.some((pick) => Number(pick.value) === id) &&
        !group.members.includes(id)
    )
        target.picks.push({ ...option, value: id })
    target.pickerValue = null
}
function unpickMember(group: Group, value: string | number) {
    const target = draft(group)
    target.picks = target.picks.filter((pick) => pick.value !== value)
}
const loadAccountOptions = async (query: string, signal: AbortSignal): Promise<PickerOption[]> => {
    const page = await searchAccounts(query, signal)
    return page.items.map(accountOption)
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
    const path = selected.value ? '/' + selected.value.group_id : ''
    const body = {
        name: form.name.trim(),
        description: form.description.trim() || null,
        ...(selected.value ? { expected_revision: selected.value.revision } : {}),
    }
    const saved = await mutation.run(
        () => groupChange(path, selected.value ? 'PATCH' : 'POST', body, key({ path, body })),
        async () => {
            await list.refresh()
            emitToast({ type: 'success', message: selected.value ? '用户组已更新' : '用户组已创建' })
        },
    )
    if (saved) {
        selected.value = undefined
        editorOpen.value = false
        form.name = ''
        form.description = ''
    }
}
async function remove(group: Group) {
    const path = '/' + group.group_id, body = { expected_revision: group.revision }
    const confirmed = await mutation.confirmRow(group.group_id, {
        title: '删除用户组',
        content: `删除“${group.name}”会移除该组的成员关系与访问来源。组内成员账户不会被删除。`,
        confirmText: '删除组并移除访问来源',
        destructive: true,
    }, () => groupChange(path, 'DELETE', body, key({ path, body })))
    if (confirmed) {
        selected.value = undefined
        await list.refresh()
        emitToast({ type: 'success', message: '用户组已删除' })
    }
}

async function addMembers(group: Group) {
    const target = draft(group)
    const picks = [...target.picks]
    if (!picks.length) return
    const added = await mutation.runRow(
        `${group.group_id}:add`,
        async () => {
            let revision = group.revision
            for (const pick of picks) {
                const path = `/${group.group_id}/members/${Number(pick.value)}`
                const body = { present: true, expected_revision: revision }
                const receipt = await groupChange(path, 'PATCH', body, key({ path, body }))
                const next = Number(receipt.summary.revision)
                if (Number.isFinite(next)) revision = next
            }
        },
        async () => {
            target.picks = []
            await list.refresh()
            emitToast({
                type: 'success',
                message: picks.length > 1 ? `已添加 ${picks.length} 名成员` : '成员已添加',
            })
        },
    )
    if (!added) await list.refresh()
}
async function removeMember(group: Group, profile: GroupMember) {
    const name = profile.nickname || profile.username || `账户 ${profile.user_id}`
    const path = `/${group.group_id}/members/${profile.user_id}`
    const body = { present: false, expected_revision: group.revision }
    const confirmed = await mutation.confirmRow(
        `${group.group_id}:remove:${profile.user_id}`,
        {
            title: '移除组成员',
            content: `将把 ${name} 移出“${group.name}”。该成员经由此组获得的应用访问会一并失效，账户本身不受影响。`,
            confirmText: '移除成员',
            destructive: true,
        },
        () => groupChange(path, 'PATCH', body, key({ path, body })),
    )
    if (confirmed) {
        await list.refresh()
        emitToast({ type: 'success', message: `已将 ${name} 移出用户组` })
    }
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
        <FailureFeedback :failure="mutation.failure.value" :message="writeError"
            :recovery-label="['reload', 'check-result'].includes(mutation.failure.value?.recovery ?? '') ? '重新加载并核对' : undefined"
            :recover="() => list.refresh()" />

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
                已加载 {{ items.length }} 个用户组{{ hasMore ? '，还有更多' : '' }}
            </span>
        </div>

        <DataState
            :status="status"
            :error="error"
            :refreshing="refreshing"
            :empty="!items.length"
            :empty-title="filters.values.value.q ? '没有匹配的用户组' : '还没有用户组'"
            :empty-description="filters.values.value.q ? '换个关键词再试试。' : '创建第一个用户组，把成员和访问来源集中管理。'"
            :skeleton-rows="3"
            @retry="list.refresh()"
        >
            <template #actions>
                <UiButton v-if="!filters.values.value.q" @click="create">
                    <Plus :size="14" aria-hidden="true" />新建用户组
                </UiButton>
            </template>
            <div class="grid gap-4 md:grid-cols-2">
                <article v-for="group in items" :key="group.group_id" class="panel">
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
                                <RouterLink
                                    v-for="app in group.application_labels || []"
                                    :key="app.application_id"
                                    class="status-badge status-brand"
                                    :to="{ name: 'admin-app', params: { id: app.application_id } }"
                                >{{ app.name }}</RouterLink>
                                <span v-if="!group.application_ids.length" class="text-[11px] text-muted-foreground">尚未分配应用访问</span>
                            </div>
                        </div>
                        <div>
                            <p class="mb-2 flex items-center gap-1.5 text-[10.5px] font-semibold uppercase tracking-wider text-muted-foreground">
                                <Users :size="11" aria-hidden="true" />成员 · {{ group.members.length }}
                            </p>
                            <div class="flex flex-wrap gap-1.5">
                                <span
                                    v-for="profile in group.member_profiles || group.members.map(id => ({ user_id: id, username: '', nickname: '' }))"
                                    :key="profile.user_id"
                                    class="inline-flex max-w-full items-center gap-1.5 rounded-md border border-border bg-muted/40 px-2 py-1 text-[10.5px]"
                                >
                                    <span class="truncate">{{ profile.nickname || profile.username || '账户 ' + profile.user_id }}</span>
                                    <span v-if="profile.username" class="truncate opacity-60">@{{ profile.username }}</span>
                                    <button
                                        type="button"
                                        class="rounded px-0.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                                        :disabled="pending"
                                        :aria-label="'移除成员 ' + (profile.nickname || profile.username || profile.user_id)"
                                        title="移除成员"
                                        @click="removeMember(group, profile)"
                                    >×</button>
                                </span>
                                <span v-if="!group.members.length" class="text-[11px] text-muted-foreground">暂无成员</span>
                            </div>
                        </div>
                        <form class="space-y-2 border-t border-border pt-4" @submit.prevent="addMembers(group)">
                            <UiFormField :id="'group-member-' + group.group_id" label="搜索并添加成员" class="min-w-0" hint="可连续选择多人后一次添加。">
                                <EntityPicker
                                    :id="'group-member-' + group.group_id"
                                    :model-value="draft(group).pickerValue"
                                    :load="loadAccountOptions"
                                    placeholder="搜索账户名称或用户名…"
                                    :disabled="pending"
                                    @update:model-value="(value) => { draft(group).pickerValue = value === null ? null : Number(value) }"
                                    @select="(option) => pickMember(group, option)"
                                />
                            </UiFormField>
                            <div v-if="draft(group).picks.length" class="flex flex-wrap gap-1.5">
                                <span v-for="pick in draft(group).picks" :key="pick.value" class="inline-flex items-center gap-1 rounded-md border border-primary/30 bg-primary/5 px-2 py-1 text-[10.5px]">
                                    {{ pick.label }}
                                    <button type="button" class="ml-0.5 text-muted-foreground hover:text-destructive" :aria-label="'取消选择 ' + pick.label" @click="unpickMember(group, pick.value)">×</button>
                                </span>
                            </div>
                            <UiButton
                                type="submit"
                                variant="outline"
                                size="sm"
                                :disabled="pending || !draft(group).picks.length"
                                :loading="rowPending(`${group.group_id}:add`)"
                            >
                                <UserPlus :size="13" aria-hidden="true" />
                                {{ draft(group).picks.length > 1 ? `添加 ${draft(group).picks.length} 名成员` : '添加成员' }}
                            </UiButton>
                        </form>
                    </div>
                </article>
            </div>
            <div v-if="hasMore" class="mt-4 flex justify-center">
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
