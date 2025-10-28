<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import Pagination from '@/components/pagination.vue'
import { useRoute, useRouter } from 'vue-router'
import { getRolesByPage, addRole, patchRole, addRoleAuthority, deleteRoleAuthority } from '../api'
import * as authorityApi from '@/pages/authority/api'
import { emitToast } from '@/utils/toast'
import type { RoleVO } from '@/types/models'
import type { Authority } from '@/pages/authority/api/type'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'

const loading = ref(false)
const roles = ref<RoleVO[]>([])
const authorities = ref<Authority[]>([])
const creating = ref(false)
const editing = ref(false)
const assigning = ref(false)
const currentRole = ref<RoleVO | null>(null)
// 创建表单
const createForm = ref<{ name: string; role_enum: string }>({ name: '', role_enum: 'Admin' })
// 编辑表单
const editForm = ref<{ id: string; name: string; role_enum: string }>({ id: '', name: '', role_enum: 'Admin' })

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

async function loadAuthorities() {
  try {
    authorities.value = await authorityApi.getAllAuthorities()
  } catch (e) {
    console.error('加载权限失败', e)
  }
}

function openCreateDialog() {
  createForm.value = { name: '', role_enum: 'Admin' }
  creating.value = true
}

async function submitCreate() {
  if (!createForm.value.name || !createForm.value.role_enum) {
    emitToast({ type: 'warning', message: '请填写角色名称与枚举' })
    return
  }
  try {
    const vo = await addRole({ name: createForm.value.name, role_enum: createForm.value.role_enum })
    emitToast({ type: 'success', message: `已创建角色：${vo.name}` })
    creating.value = false
    await loadRoles()
  } catch (e) {
    // 错误已由拦截器弹出，这里补充提示
    emitToast({ type: 'error', message: '创建角色失败，可能是枚举或名称重复' })
  }
}

function openEditDialog(role: RoleVO) {
  currentRole.value = role
  editForm.value = { id: role.id, name: role.name, role_enum: role.role_enum }
  editing.value = true
}

async function submitEdit() {
  try {
    const vo = await patchRole({ id: editForm.value.id, name: editForm.value.name, role_enum: editForm.value.role_enum })
    emitToast({ type: 'success', message: `已更新角色名称：${vo.name}` })
    editing.value = false
    await loadRoles()
  } catch (e) {
    emitToast({ type: 'error', message: '更新角色失败，可能是名称重复' })
  }
}

async function openAssignDialog(role: RoleVO) {
  currentRole.value = role
  assigning.value = true
  await loadAuthorities()
}

async function grantAuthority(authorityId: string) {
  if (!currentRole.value) return
  const ok = await addRoleAuthority({ role_id: currentRole.value.id, authority_id: authorityId })
  emitToast({ type: ok ? 'success' : 'error', message: ok ? '授予成功' : '授予失败' })
}

async function revokeAuthority(authorityId: string) {
  if (!currentRole.value) return
  const ok = await deleteRoleAuthority({ role_id: currentRole.value.id, authority_id: authorityId })
  emitToast({ type: ok ? 'success' : 'error', message: ok ? '移除成功' : '移除失败' })
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
  <div>
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-xl font-semibold">角色列表</h2>
      <div class="flex items-center gap-2">
        <UiButton size="sm" @click="openCreateDialog">创建角色</UiButton>
      </div>
    </div>

    <div class="overflow-x-auto bg-card text-card-foreground rounded-md shadow-sm border border-border">
      <table class="min-w-full divide-y divide-border text-sm">
        <thead>
          <tr>
            <th class="px-4 py-2 text-left">角色名称</th>
            <th class="px-4 py-2 text-left">类型</th>
            <th class="px-4 py-2 text-left">创建时间</th>
            <th class="px-4 py-2 text-left">更新时间</th>
            <th class="px-4 py-2 text-left">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in roles" :key="r.id" class="border-t border-border">
            <td class="px-4 py-2">{{ r.name }}</td>
            <td class="px-4 py-2">{{ r.role_enum }}</td>
            <td class="px-4 py-2">{{ r.create_time }}</td>
            <td class="px-4 py-2">{{ r.update_time }}</td>
            <td class="px-4 py-2">
              <div class="flex gap-2">
                <UiButton size="sm" variant="ghost" :disabled="r.role_enum==='Superadmin'" @click="openEditDialog(r)">重命名</UiButton>
                <UiButton size="sm" variant="ghost" :disabled="r.role_enum==='Superadmin'" @click="openAssignDialog(r)">权限</UiButton>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-if="loading" class="p-4 text-center text-sm text-muted-foreground">正在加载...</div>
    </div>

    <Pagination
      :page-number="pageNumber"
      :total-pages="totalPages"
      :page-size="pageSize"
      @change="(p:number) => { pageNumber = p; updateQuery(); loadRoles(); }"
      @size-change="(s:number) => { pageSize = s; pageNumber = 1; updateQuery(); loadRoles(); }"
    />

    <!-- 创建角色对话框（shadcn/ui） -->
    <UiDialog v-model:open="creating" title="创建角色" @confirm="submitCreate" @cancel="() => (creating = false)">
      <div class="space-y-3">
        <div>
          <label class="block text-sm mb-1">角色名称</label>
          <UiInput v-model="createForm.name" placeholder="例如：管理员" />
        </div>
        <div>
          <label class="block text-sm mb-1">角色枚举</label>
          <UiSelect v-model="createForm.role_enum">
            <option value="Admin">Admin</option>
            <option value="User">User</option>
            <option value="Superadmin">Superadmin</option>
          </UiSelect>
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
        <p class="text-xs text-muted-foreground">枚举：{{ editForm.role_enum }}（不建议修改）</p>
      </div>
    </UiDialog>

    <!-- 权限分配对话框（shadcn/ui） -->
    <UiDialog v-model:open="assigning" :title="`角色权限：${currentRole?.name || ''}`" @cancel="() => (assigning=false)">
      <div class="overflow-x-auto">
        <table class="min-w-full divide-y divide-border text-sm">
          <thead>
            <tr>
              <th class="px-3 py-2 text-left">权限名称</th>
              <th class="px-3 py-2 text-left">URI</th>
              <th class="px-3 py-2 text-left">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="a in authorities" :key="a.id" class="border-t border-border">
              <td class="px-3 py-2">{{ a.name }}</td>
              <td class="px-3 py-2 font-mono text-xs">{{ a.authority }}</td>
              <td class="px-3 py-2">
                <div class="flex gap-2">
                  <UiButton size="sm" variant="ghost" @click="grantAuthority(a.id)">授予</UiButton>
                  <UiButton size="sm" variant="ghost" @click="revokeAuthority(a.id)">移除</UiButton>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </UiDialog>
  </div>
</template>
