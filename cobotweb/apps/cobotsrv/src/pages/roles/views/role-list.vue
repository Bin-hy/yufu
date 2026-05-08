<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
import { useRoute, useRouter } from 'vue-router'
import { getRolesByPage, addRole, patchRole} from '../api'
import { emitToast } from '@/utils/toast'
import type { RoleVO } from '@/types/models'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'

const loading = ref(false)
const roles = ref<RoleVO[]>([])
const creating = ref(false)
const editing = ref(false)
const currentRole = ref<RoleVO | null>(null)
// 创建表单
const createForm = ref<{ name: string;  }>({ name: '' })
// 编辑表单
const editForm = ref<{ id: number; name: string;  }>({ id: 0, name: '' })

// 分页
const pageNumber = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)

const router = useRouter()
const route = useRoute()
function updateQuery() {
  router.replace({ query: { ...route.query, page_number: pageNumber.value, page_size: pageSize.value } })
}

async function loadRoles() {
  loading.value = true
  try {
    const res = await getRolesByPage(pageNumber.value, pageSize.value)
    roles.value = res.data_list || []
    totalPages.value = res.total_pages || 1
  } catch (e) {
    console.error('加载角色失败', e)
  } finally {
    loading.value = false
  }
}

function openCreateDialog() {
  createForm.value = { name: '' }
  creating.value = true
}

async function submitCreate() {
  if (!createForm.value.name ) {
    emitToast({ type: 'warning', message: '请填写角色名称' })
    return
  }
  try {
    const vo = await addRole({ name: createForm.value.name })
    emitToast({ type: 'success', message: `已创建角色：${vo.name}` })
    creating.value = false
    await loadRoles()
  } catch (e) {
    // 错误已由拦截器弹出，这里补充提示
    emitToast({ type: 'error', message: '创建角色失败，可能是名称重复' })
  }
}

function openEditDialog(role: RoleVO) {
  currentRole.value = role
  editForm.value = { id: role.id, name: role.name }
  editing.value = true
}

async function submitEdit() {
  try {
    const vo = await patchRole({ id: editForm.value.id, name: editForm.value.name })
    emitToast({ type: 'success', message: `已更新角色名称：${vo.name}` })
    editing.value = false
    await loadRoles()
  } catch (e) {
    emitToast({ type: 'error', message: '更新角色失败，可能是名称重复' })
  }
}

onMounted(async () => {
  const pn = Number(route.query.page_number ?? 1)
  const ps = Number(route.query.page_size ?? 10)
  pageNumber.value = Number.isFinite(pn) && pn > 0 ? pn : 1
  pageSize.value = [10, 20, 50].includes(ps) ? ps : 10
  updateQuery()
  await loadRoles()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h2 class="text-2xl font-bold text-foreground">角色管理</h2>
        <p class="text-sm text-muted-foreground mt-1">维护系统角色与权限分配</p>
      </div>
      <div class="flex items-center gap-3">
        <UiButton size="sm" @click="openCreateDialog">
          <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"></path>
          </svg>
          创建角色
        </UiButton>
      </div>
    </div>

    <div class="bg-card rounded-lg border border-border shadow-sm">
      <div class="px-6 py-4 border-b border-border">
        <div class="flex items-center justify-between">
          <h3 class="text-lg font-medium text-foreground">角色列表</h3>
          <div class="text-sm text-muted-foreground">共 {{ roles.length }} 个角色</div>
        </div>
      </div>
      <div class="overflow-x-auto">
        <table class="w-full">
          <thead>
            <tr class="border-b border-border bg-muted/50">
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">角色名称</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">类型</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">创建时间</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">更新时间</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            <tr v-for="r in roles" :key="r.id" class="hover:bg-muted/30 transition-colors">
              <td class="px-6 py-4">
                <div class="flex items-center gap-3">
                  <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                    <svg class="w-5 h-5 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5.121 17.804A1 1 0 014 17V7a1 1 0 011.121-.804l13 3A1 1 0 0119 10v4a1 1 0 01-.879.993l-13 2.81z"></path>
                    </svg>
                  </div>
                  <div>
                    <div class="text-sm font-medium text-foreground">{{ r.name }}</div>
                    <div class="text-xs text-muted-foreground">ID: {{ r.id }}</div>
                  </div>
                </div>
              </td>
              <td class="px-6 py-4">{{ r.create_time }}</td>
              <td class="px-6 py-4">{{ r.update_time }}</td>
              <td class="px-6 py-4">
                <div class="flex gap-2">
                  <UiButton size="sm" variant="outline" @click="openEditDialog(r)">重命名</UiButton>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
        <div v-if="!loading && roles.length === 0" class="text-center py-12">
          <svg class="w-12 h-12 text-muted-foreground mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 1.343-3 3s1.343 3 3 3 3-1.343 3-3-1.343-3-3-3z"></path>
          </svg>
          <h3 class="text-lg font-medium text-foreground mb-2">暂无角色数据</h3>
          <p class="text-muted-foreground">请先创建角色以进行权限管理</p>
        </div>
      </div>
      <div v-if="loading" class="p-4 text-center text-sm text-muted-foreground">正在加载...</div>
    </div>

    <div class="flex justify-center">
      <Pagination
        :page="pageNumber"
        :page-count="totalPages"
        :size="pageSize"
        @page-change="(p:number) => { pageNumber = p; updateQuery(); loadRoles(); }"
        @update:size="(s:number) => { pageSize = s; pageNumber = 1; updateQuery(); loadRoles(); }"
      />
    </div>

    <!-- 创建角色对话框（shadcn/ui） -->
    <UiDialog v-model:open="creating" title="创建角色" @confirm="submitCreate" @cancel="() => (creating = false)">
      <div class="space-y-3">
        <div>
          <label class="block text-sm mb-1">角色名称</label>
          <UiInput v-model="createForm.name" placeholder="例如：管理员" />
        </div>
      </div>
    </UiDialog>

    <!-- 重命名对话框（shadcn/ui） -->
    <UiDialog v-model:open="editing" title="重命名角色" @confirm="submitEdit" @cancel="() => (editing = false)">
      <div class="space-y-3">
        <div>
          <label class="block text-sm mb-1">角色名称</label>
          <UiInput v-model="editForm.name" />
        </div>
      </div>
    </UiDialog>
  </div>
</template>
