<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { emitToast } from '@/utils/toast'
import { getApplicationsByPage, createApplication, patchApplication, deleteApplication } from '../api'
import type { ApplicationVO } from '@/types/models'
import Pagination from '@/components/ui/pagination/pagination.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'

const loading = ref(false)
const applications = ref<ApplicationVO[]>([])
const pageNumber = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)

const router = useRouter()
const route = useRoute()
function updateQuery() {
  router.replace({ query: { ...route.query, page_number: pageNumber.value, page_size: pageSize.value } })
}

async function loadApplications() {
  loading.value = true
  try {
    const res = await getApplicationsByPage(pageNumber.value, pageSize.value)
    applications.value = res.data_list || []
    totalPages.value = res.total_pages || 1
  } catch (e) {
    console.error('加载应用列表失败', e)
  } finally {
    loading.value = false
  }
}

// --- create ---
const creating = ref(false)
const createForm = ref({ app_key: '', name: '', description: '', oidc_client_id: '' })

function openCreateDialog() {
  createForm.value = { app_key: '', name: '', description: '', oidc_client_id: '' }
  creating.value = true
}

async function submitCreate() {
  if (!createForm.value.app_key.trim() || !createForm.value.name.trim()) {
    emitToast({ type: 'warning', message: '请填写应用标识和名称' })
    return
  }
  try {
    const vo = await createApplication({
      app_key: createForm.value.app_key.trim(),
      name: createForm.value.name.trim(),
      description: createForm.value.description.trim() || null,
      oidc_client_id: createForm.value.oidc_client_id ? Number(createForm.value.oidc_client_id) : null,
    })
    emitToast({ type: 'success', message: `已创建应用：${vo.name}` })
    creating.value = false
    await loadApplications()
  } catch (e) {
    emitToast({ type: 'error', message: '创建应用失败，可能是 app_key 重复' })
  }
}

// --- edit ---
const editing = ref(false)
const editForm = ref({ id: 0, name: '', description: '', oidc_client_id: '' })

function openEditDialog(app: ApplicationVO) {
  editForm.value = {
    id: app.id,
    name: app.name,
    description: app.description || '',
    oidc_client_id: app.oidc_client_id ? String(app.oidc_client_id) : '',
  }
  editing.value = true
}

async function submitEdit() {
  try {
    const vo = await patchApplication({
      id: editForm.value.id,
      name: editForm.value.name.trim() || null,
      description: editForm.value.description.trim() || null,
      oidc_client_id: editForm.value.oidc_client_id ? Number(editForm.value.oidc_client_id) : null,
    })
    emitToast({ type: 'success', message: `已更新应用：${vo.name}` })
    editing.value = false
    await loadApplications()
  } catch (e) {
    emitToast({ type: 'error', message: '更新应用失败' })
  }
}

// --- delete ---
const deleting = ref(false)
const appToDelete = ref<ApplicationVO | null>(null)

function openDeleteDialog(app: ApplicationVO) {
  appToDelete.value = app
  deleting.value = true
}

async function confirmDelete() {
  if (!appToDelete.value) return
  try {
    await deleteApplication(appToDelete.value.id)
    emitToast({ type: 'success', message: `已删除应用：${appToDelete.value.name}` })
    deleting.value = false
    appToDelete.value = null
    await loadApplications()
  } catch (e) {
    emitToast({ type: 'error', message: '删除应用失败' })
  }
}

onMounted(async () => {
  const pn = Number(route.query.page_number ?? 1)
  const ps = Number(route.query.page_size ?? 10)
  pageNumber.value = Number.isFinite(pn) && pn > 0 ? pn : 1
  pageSize.value = [10, 20, 50].includes(ps) ? ps : 10
  updateQuery()
  await loadApplications()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h2 class="text-2xl font-bold text-foreground">应用注册</h2>
        <p class="text-sm text-muted-foreground mt-1">管理接入平台的业务应用</p>
      </div>
      <div class="flex items-center gap-3">
        <UiButton @click="openCreateDialog">
          <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"></path>
          </svg>
          注册应用
        </UiButton>
      </div>
    </div>

    <div class="bg-card rounded-lg border border-border shadow-sm">
      <div class="px-6 py-4 border-b border-border">
        <div class="flex items-center justify-between">
          <h3 class="text-lg font-medium text-foreground">应用列表</h3>
          <div class="text-sm text-muted-foreground">共 {{ applications.length }} 个应用</div>
        </div>
      </div>
      <div class="overflow-x-auto">
        <table class="w-full">
          <thead>
            <tr class="border-b border-border bg-muted/50">
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">应用标识</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">名称</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">描述</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">OIDC Client</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">创建时间</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            <tr v-for="app in applications" :key="app.id" class="hover:bg-muted/30 transition-colors">
              <td class="px-6 py-4">
                <span class="text-sm font-mono text-primary bg-primary/10 px-2 py-0.5 rounded">{{ app.app_key }}</span>
              </td>
              <td class="px-6 py-4 text-sm text-foreground">{{ app.name }}</td>
              <td class="px-6 py-4 text-sm text-muted-foreground">{{ app.description || '-' }}</td>
              <td class="px-6 py-4 text-sm text-muted-foreground">{{ app.oidc_client_id || '-' }}</td>
              <td class="px-6 py-4 text-sm text-muted-foreground">{{ app.create_time }}</td>
              <td class="px-6 py-4">
                <div class="flex gap-2">
                  <UiButton size="sm" variant="outline" @click="openEditDialog(app)">编辑</UiButton>
                  <UiButton size="sm" variant="destructive" @click="openDeleteDialog(app)">删除</UiButton>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-if="!loading && applications.length === 0" class="text-center py-12">
          <svg class="w-12 h-12 text-muted-foreground mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"></path>
          </svg>
          <h3 class="text-lg font-medium text-foreground mb-2">暂无应用数据</h3>
          <p class="text-muted-foreground">请先注册应用以使用权限管理</p>
        </div>
      </div>
      <div v-if="loading" class="p-4 text-center text-sm text-muted-foreground">正在加载...</div>
    </div>

    <div class="flex justify-center">
      <Pagination
        :page="pageNumber"
        :page-count="totalPages"
        :size="pageSize"
        @page-change="(p:number) => { pageNumber = p; updateQuery(); loadApplications(); }"
        @update:size="(s:number) => { pageSize = s; pageNumber = 1; updateQuery(); loadApplications(); }"
      />
    </div>

    <!-- 创建对话框 -->
    <UiDialog v-model:open="creating" title="注册应用" @confirm="submitCreate" @cancel="() => (creating = false)">
      <div class="space-y-3">
        <div>
          <label class="block text-sm mb-1">应用标识 <span class="text-destructive">*</span></label>
          <UiInput v-model="createForm.app_key" placeholder="例如：cobotrix" />
        </div>
        <div>
          <label class="block text-sm mb-1">名称 <span class="text-destructive">*</span></label>
          <UiInput v-model="createForm.name" placeholder="例如：CoBotrix 管理端" />
        </div>
        <div>
          <label class="block text-sm mb-1">描述</label>
          <UiInput v-model="createForm.description" placeholder="可选描述" />
        </div>
        <div>
          <label class="block text-sm mb-1">OIDC Client ID</label>
          <UiInput v-model="createForm.oidc_client_id" placeholder="关联的 OAuth 客户端 ID（可选）" />
        </div>
      </div>
    </UiDialog>

    <!-- 编辑对话框 -->
    <UiDialog v-model:open="editing" title="编辑应用" @confirm="submitEdit" @cancel="() => (editing = false)">
      <div class="space-y-3">
        <div>
          <label class="block text-sm mb-1">名称</label>
          <UiInput v-model="editForm.name" />
        </div>
        <div>
          <label class="block text-sm mb-1">描述</label>
          <UiInput v-model="editForm.description" />
        </div>
        <div>
          <label class="block text-sm mb-1">OIDC Client ID</label>
          <UiInput v-model="editForm.oidc_client_id" />
        </div>
      </div>
    </UiDialog>

    <!-- 删除确认对话框 -->
    <UiDialog v-model:open="deleting" title="确认删除" @confirm="confirmDelete" @cancel="() => { deleting = false; appToDelete = null }">
      <p class="text-sm text-muted-foreground">确认删除应用「{{ appToDelete?.name || '' }}」？该操作不可恢复。</p>
    </UiDialog>
  </div>
</template>
