<script setup lang="ts">
defineOptions({ name: "AdminGroups" })
import { reactive, ref } from 'vue'
import { listGroups, groupChange } from '../api'
import type { Group } from '../api/type'
import { useCursorPage } from '@/composables/use-cursor-page'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useOperationKey } from '@/composables/use-ephemeral-result'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
const list = useCursorPage('enterprise-groups', listGroups), { data, status, error } = list
const mutation = useAsyncMutation(), { pending, error: writeError } = mutation, key = useOperationKey()
const form = reactive({ name: '', description: '' }), selected = ref<Group>(), member = ref(0)
async function change(path: string, method: 'POST' | 'PATCH' | 'DELETE', body: unknown) {
    await mutation.run(() => groupChange(path, method, body, key({ path, body })), async () => { selected.value = undefined; await list.refresh() })
}
function edit(group: Group) { selected.value = group; form.name = group.name; form.description = group.description ?? '' }
async function save() { await change(selected.value ? '/' + selected.value.group_id : '', selected.value ? 'PATCH' : 'POST', { name: form.name.trim(), description: form.description.trim() || null, ...(selected.value ? { expected_revision: selected.value.revision } : {}) }); form.name = ''; form.description = '' }
</script>
<template>
    <div class="workspace-page"><PageHeader title="用户组" description="扁平用户组仅汇总已明确分配的应用访问。" /><p v-if="writeError" class="inline-error mb-4" role="alert">{{ writeError }}</p>
        <form class="mb-5 flex flex-wrap items-end gap-3 rounded-xl border bg-card p-5" @submit.prevent="save"><div class="form-field"><label for="group-name">{{ selected ? '编辑组名称' : '新组名称' }}</label><UiInput id="group-name" v-model="form.name" required :disabled="pending" /></div><div class="form-field"><label for="group-description">说明</label><UiInput id="group-description" v-model="form.description" :disabled="pending" /></div><UiButton type="submit" :disabled="pending">{{ selected ? '保存用户组' : '创建用户组' }}</UiButton><UiButton v-if="selected" variant="outline" @click="selected = undefined; form.name = ''; form.description = ''">取消编辑</UiButton></form>
        <DataState :status="status" :error="error" :empty="!data?.items.length" @retry="list.refresh"><div class="space-y-4"><article v-for="group in data?.items" :key="group.group_id" class="rounded-xl border bg-card p-5"><div class="flex flex-wrap justify-between gap-3"><div><h2 class="font-semibold">{{ group.name }}</h2><p class="mt-2 select-all text-xs text-muted-foreground">{{ group.group_id }}</p></div><div class="flex gap-2"><UiButton variant="outline" :disabled="pending" @click="edit(group)">编辑</UiButton><UiButton variant="outline" :disabled="pending" @click="change('/' + group.group_id, 'DELETE', { expected_revision: group.revision })">删除组并移除访问来源</UiButton></div></div><p class="mt-3 text-xs">已分配应用 {{ group.application_ids.join('、') || '无' }}</p><div class="mt-3 flex flex-wrap gap-2"><UiButton v-for="userId in group.members" :key="userId" variant="outline" :disabled="pending" @click="change(`/${group.group_id}/members/${userId}`, 'PATCH', { present: false, expected_revision: group.revision })">账户 {{ userId }} · 移除</UiButton></div><form class="mt-4 flex items-end gap-3" @submit.prevent="change(`/${group.group_id}/members/${member}`, 'PATCH', { present: true, expected_revision: group.revision })"><div class="form-field"><label :for="'group-member-' + group.group_id">新成员账户 ID</label><UiInput :id="'group-member-' + group.group_id" v-model.number="member" type="number" min="1" required /></div><UiButton type="submit" :disabled="pending || !member">添加成员</UiButton></form></article></div></DataState><UiButton v-if="data?.next" class="mt-4" variant="outline" @click="list.next">下一页</UiButton>
    </div>
</template>
