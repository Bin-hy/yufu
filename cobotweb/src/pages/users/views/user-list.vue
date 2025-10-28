<script lang="ts" setup>
import { ref, computed, onMounted } from 'vue'
import { emitToast } from '@/utils/toast'
import { getUsersByPage, getModifiableRoles, patchUserRole, deleteRole as deleteRoleApi } from '../api'
import type { RoleVO, UserVO } from '@/types/models'
import Pagination from '@/components/pagination.vue'
import { useRoute, useRouter } from 'vue-router'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'

// 状态
const loading = ref(false)
const users = ref<UserVO[]>([])
const roles = ref<RoleVO[]>([])
const searchQuery = ref('')

// 分页
const pageNumber = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)

const router = useRouter()
const route = useRoute()
function updateQuery() {
  router.replace({ query: { ...route.query, page_number: pageNumber.value, page_size: pageSize.value } })
}

// 正在保存的用户（用于禁用按钮）
const savingUserId = ref<string | null>(null)

const filteredUsers = computed(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return users.value
  return users.value.filter(u =>
    (u.username || '').toLowerCase().includes(q) ||
    (u.nickname || '').toLowerCase().includes(q) ||
    (u.role?.name || '').toLowerCase().includes(q)
  )
})

async function loadUsers() {
  loading.value = true
  try {
    const res = await getUsersByPage(pageNumber.value, pageSize.value)
    users.value = res.data_list || []
    totalPages.value = res.total_pages || 1
  } catch (e) {
    console.error('加载用户失败', e)
  } finally {
    loading.value = false
  }
}

async function loadRoles() {
  try {
    const res = await getModifiableRoles(1, 100)
    roles.value = res.data_list || []
  } catch (e) {
    console.error('加载角色失败', e)
  }
}

async function changeUserRole(user: UserVO, newRoleId: string) {
  savingUserId.value = user.id || null
  try {
    const updated = await patchUserRole({ user_id: user.id!, role_id: newRoleId })
    emitToast({ type: 'success', message: '角色更新成功' })
    user.role = updated.role
  } catch (e) {
    console.error('更新角色失败', e)
    emitToast({ type: 'error', message: '更新角色失败' })
  } finally {
    savingUserId.value = null
  }
}

async function deleteRole(roleId?: string) {
  if (!roleId) return
  try {
    await deleteRoleApi(roleId)
    emitToast({ type: 'success', message: '角色已删除' })
    await loadUsers()
    await loadRoles()
  } catch (e) {
    console.error('删除角色失败', e)
    emitToast({ type: 'error', message: '删除角色失败' })
  }
}

onMounted(async () => {
  const pn = Number(route.query.page_number ?? 1)
  const ps = Number(route.query.page_size ?? 10)
  pageNumber.value = Number.isFinite(pn) && pn > 0 ? pn : 1
  pageSize.value = [10, 20, 50].includes(ps) ? ps : 10
  updateQuery()
  await loadUsers()
  await loadRoles()
})
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-xl font-semibold">用户列表</h2>
      <UiInput v-model="searchQuery" placeholder="搜索用户名/昵称/角色" :class="'h-8 w-64 px-2 text-sm'" />
    </div>

    <div class="overflow-x-auto bg-card text-card-foreground rounded-md shadow-sm border border-border">
      <table class="min-w-full">
        <thead>
          <tr class="border-b">
            <th class="px-4 py-2 text-left">用户名</th>
            <th class="px-4 py-2 text-left">昵称</th>
            <th class="px-4 py-2 text-left">角色</th>
            <th class="px-4 py-2 text-left">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="u in filteredUsers" :key="u.id" class="border-t">
            <td class="px-4 py-2">{{ u.username }}</td>
            <td class="px-4 py-2">{{ u.nickname || '-' }}</td>
            <td class="px-4 py-2">
              <div class="flex items-center gap-2">
                <UiSelect :model-value="u.role?.id" class="h-8 text-sm w-56" @update:modelValue="(v:any) => changeUserRole(u, v)">
                  <option v-for="r in roles" :key="r.id" :value="r.id">{{ r.name }} ({{ r.role_enum }})</option>
                </UiSelect>
                <UiButton size="sm" :disabled="savingUserId === u.id">保存</UiButton>
              </div>
            </td>
            <td class="px-4 py-2">
              <UiButton size="sm" variant="destructive" :disabled="u.role?.role_enum?.toUpperCase() === 'SUPERADMIN'" @click="deleteRole(u.role?.id)">删除该角色</UiButton>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 分页 -->
    <Pagination
      :page-number="pageNumber"
      :total-pages="totalPages"
      :page-size="pageSize"
      @change="(p:number) => { pageNumber = p; updateQuery(); loadUsers(); }"
      @size-change="(s:number) => { pageSize = s; pageNumber = 1; updateQuery(); loadUsers(); }"
    />
  </div>
</template>
