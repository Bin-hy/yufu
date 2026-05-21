<script lang="ts" setup>
import { ref, reactive, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { emitToast } from '@/utils/toast'
import * as permApi from '../api'
import * as appApi from '@/pages/applications/api'
import type { PermissionVO, ApplicationVO } from '@/types/models'
import Pagination from '@/components/ui/pagination/pagination.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import UiSelect from '@/components/ui/select/select.vue'

const loading = ref(false)
const permissions = ref<PermissionVO[]>([])
const applications = ref<ApplicationVO[]>([])
const filterAppId = ref<number | null>(null)
const pageNumber = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)

const router = useRouter()
const route = useRoute()
function updateQuery() {
  router.replace({ query: { ...route.query, page_number: pageNumber.value, page_size: pageSize.value } })
}

async function loadPermissions() {
  loading.value = true
  try {
    const res = await permApi.getPermissionsByPage(pageNumber.value, pageSize.value, filterAppId.value)
    permissions.value = res.data_list || []
    totalPages.value = res.total_pages || 1
  } catch (e) {
    console.error('加载权限列表失败', e)
  } finally {
    loading.value = false
  }
}

async function loadApplications() {
  try {
    const res = await appApi.getApplicationsByPage(1, 100)
    applications.value = res.data_list || []
  } catch (e) {
    console.error('加载应用列表失败', e)
  }
}

function onFilterChange() {
  pageNumber.value = 1
  updateQuery()
  loadPermissions()
}

// --- create ---
const creating = ref(false)
const createForm = reactive({
  application_id: 0,
  name: '',
  description: '',
  action: '',
  resource_type: '',
  resource_pattern: '',
})

function openCreateDialog() {
  createForm.application_id = filterAppId.value || applications.value[0]?.id || 0
  createForm.name = ''
  createForm.description = ''
  createForm.action = ''
  createForm.resource_type = ''
  createForm.resource_pattern = ''
  creating.value = true
}

async function submitCreate() {
  if (!createForm.application_id || !createForm.name.trim() || !createForm.action.trim()) {
    emitToast({ type: 'warning', message: '请填写应用、权限名和动作' })
    return
  }
  try {
    const vo = await permApi.createPermission({
      application_id: createForm.application_id,
      name: createForm.name.trim(),
      description: createForm.description.trim() || null,
      action: createForm.action.trim(),
      resource_type: createForm.resource_type.trim() || null,
      resource_pattern: createForm.resource_pattern.trim() || null,
    })
    emitToast({ type: 'success', message: `已创建权限：${vo.name}` })
    creating.value = false
    await loadPermissions()
  } catch (e) {
    emitToast({ type: 'error', message: '创建权限失败，可能是名称重复' })
  }
}

// --- edit ---
const editing = ref(false)
const editForm = reactive({
  id: 0,
  name: '',
  description: '',
  action: '',
  resource_type: '',
  resource_pattern: '',
  application_id: 0 as number | null,
})

function openEditDialog(perm: PermissionVO) {
  editForm.id = perm.id
  editForm.name = perm.name
  editForm.description = perm.description || ''
  editForm.action = perm.action
  editForm.resource_type = perm.resource_type || ''
  editForm.resource_pattern = perm.resource_pattern || ''
  editForm.application_id = null
  editing.value = true
}

async function submitEdit() {
  try {
    const vo = await permApi.patchPermission({
      id: editForm.id,
      name: editForm.name.trim() || null,
      description: editForm.description.trim() || null,
      action: editForm.action.trim() || null,
      resource_type: editForm.resource_type.trim() || null,
      resource_pattern: editForm.resource_pattern.trim() || null,
      application_id: editForm.application_id,
    })
    emitToast({ type: 'success', message: `已更新权限：${vo.name}` })
    editing.value = false
    await loadPermissions()
  } catch (e) {
    emitToast({ type: 'error', message: '更新权限失败' })
  }
}

// --- delete ---
const deleting = ref(false)
const permToDelete = ref<PermissionVO | null>(null)

function openDeleteDialog(perm: PermissionVO) {
  permToDelete.value = perm
  deleting.value = true
}

async function confirmDelete() {
  if (!permToDelete.value) return
  try {
    await permApi.deletePermission(permToDelete.value.id)
    emitToast({ type: 'success', message: `已删除权限：${permToDelete.value.name}` })
    deleting.value = false
    permToDelete.value = null
    await loadPermissions()
  } catch (e) {
    emitToast({ type: 'error', message: '删除权限失败' })
  }
}

onMounted(async () => {
  const pn = Number(route.query.page_number ?? 1)
  const ps = Number(route.query.page_size ?? 10)
  pageNumber.value = Number.isFinite(pn) && pn > 0 ? pn : 1
  pageSize.value = [10, 20, 50].includes(ps) ? ps : 10
  updateQuery()
  await loadApplications()
  await loadPermissions()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h2 class="text-2xl font-bold text-foreground">资源权限</h2>
        <p class="text-sm text-muted-foreground mt-1">管理应用级 resource 权限规则</p>
      </div>
      <div class="flex items-center gap-3">
        <UiSelect v-if="applications.length > 0" v-model.number="filterAppId" class="w-48" @update:modelValue="onFilterChange">
          <option v-for="app in applications" :key="app.id" :value="app.id">{{ app.name }}</option>
        </UiSelect>
        <span v-else class="text-sm text-muted-foreground">暂无应用，请先<a href="/applications" class="text-primary hover:underline">注册应用</a></span>
        <UiButton @click="openCreateDialog">
          <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"></path>
          </svg>
          新增权限
        </UiButton>
      </div>
    </div>

    <div class="bg-card rounded-lg border border-border shadow-sm">
      <div class="px-6 py-4 border-b border-border">
        <div class="flex items-center justify-between">
          <h3 class="text-lg font-medium text-foreground">权限列表</h3>
          <div class="text-sm text-muted-foreground">共 {{ permissions.length }} 条</div>
        </div>
      </div>
      <div class="overflow-x-auto">
        <table class="w-full">
          <thead>
            <tr class="border-b border-border bg-muted/50">
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">权限名</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">动作</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">资源类型</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">资源匹配</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">所属应用</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            <tr v-for="perm in permissions" :key="perm.id" class="hover:bg-muted/30 transition-colors">
              <td class="px-6 py-4">
                <span class="text-sm font-mono text-primary bg-primary/10 px-2 py-0.5 rounded">{{ perm.name }}</span>
              </td>
              <td class="px-6 py-4 text-sm">{{ perm.action }}</td>
              <td class="px-6 py-4 text-sm text-muted-foreground">{{ perm.resource_type || '*' }}</td>
              <td class="px-6 py-4 text-sm text-muted-foreground">{{ perm.resource_pattern || '*' }}</td>
              <td class="px-6 py-4 text-sm text-muted-foreground">App#{{ perm.application_id }}</td>
              <td class="px-6 py-4">
                <div class="flex gap-2">
                  <UiButton size="sm" variant="outline" @click="openEditDialog(perm)">编辑</UiButton>
                  <UiButton size="sm" variant="destructive" @click="openDeleteDialog(perm)">删除</UiButton>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-if="!loading && permissions.length === 0" class="text-center py-12">
          <svg class="w-12 h-12 text-muted-foreground mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
          </svg>
          <h3 class="text-lg font-medium text-foreground mb-2">暂无权限数据</h3>
          <p class="text-muted-foreground">请先创建权限规则</p>
        </div>
      </div>
      <div v-if="loading" class="p-4 text-center text-sm text-muted-foreground">正在加载...</div>
    </div>

    <div class="flex justify-center">
      <Pagination
        :page="pageNumber"
        :page-count="totalPages"
        :size="pageSize"
        @page-change="(p:number) => { pageNumber = p; updateQuery(); loadPermissions(); }"
        @update:size="(s:number) => { pageSize = s; pageNumber = 1; updateQuery(); loadPermissions(); }"
      />
    </div>

    <!-- 创建对话框 -->
    <UiDialog v-model:open="creating" title="新增权限" @confirm="submitCreate" @cancel="() => (creating = false)">
      <div class="space-y-3">
        <div>
          <label class="block text-sm mb-1">所属应用 <span class="text-destructive">*</span></label>
          <UiSelect v-model.number="createForm.application_id" class="w-full">
            <option v-for="app in applications" :key="app.id" :value="app.id">{{ app.name }}</option>
          </UiSelect>
        </div>
        <div>
          <label class="block text-sm mb-1">权限名 <span class="text-destructive">*</span></label>
          <UiInput v-model="createForm.name" placeholder="例如：device:read" />
        </div>
        <div>
          <label class="block text-sm mb-1">动作 <span class="text-destructive">*</span></label>
          <UiInput v-model="createForm.action" placeholder="例如：read" />
        </div>
        <div>
          <label class="block text-sm mb-1">资源类型</label>
          <UiInput v-model="createForm.resource_type" placeholder="例如：device（可选）" />
        </div>
        <div>
          <label class="block text-sm mb-1">资源匹配</label>
          <UiInput v-model="createForm.resource_pattern" placeholder="例如：*（可选 glob）" />
        </div>
      </div>
    </UiDialog>

    <!-- 编辑对话框 -->
    <UiDialog v-model:open="editing" title="编辑权限" @confirm="submitEdit" @cancel="() => (editing = false)">
      <div class="space-y-3">
        <div>
          <label class="block text-sm mb-1">权限名</label>
          <UiInput v-model="editForm.name" />
        </div>
        <div>
          <label class="block text-sm mb-1">动作</label>
          <UiInput v-model="editForm.action" />
        </div>
        <div>
          <label class="block text-sm mb-1">资源类型</label>
          <UiInput v-model="editForm.resource_type" />
        </div>
        <div>
          <label class="block text-sm mb-1">资源匹配</label>
          <UiInput v-model="editForm.resource_pattern" />
        </div>
      </div>
    </UiDialog>

    <!-- 删除确认对话框 -->
    <UiDialog v-model:open="deleting" title="确认删除" @confirm="confirmDelete" @cancel="() => { deleting = false; permToDelete = null }">
      <p class="text-sm text-muted-foreground">确认删除权限「{{ permToDelete?.name || '' }}」？该操作不可恢复。</p>
    </UiDialog>
  </div>
</template>
