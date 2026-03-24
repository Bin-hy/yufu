<script lang="ts" setup>
import { ref, computed, onMounted, onActivated } from 'vue'
import { emitToast } from '@/utils/toast'
import { getUsersByPage, getModifiableRoles, patchUserRole, deleteUser } from '../api'
import type { RoleVO, UserVO } from '@/types/models'
import Pagination from '@/components/ui/pagination/pagination.vue'
import { useRoute, useRouter } from 'vue-router'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'

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

// 待提交的角色选择（键为用户ID，值为角色ID）
const pendingRole = ref<Record<string, string | undefined>>({})

// 删除确认弹窗状态
const confirmingDelete = ref(false)
const userToDelete = ref<UserVO | null>(null)

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

// 选择角色时仅记录待提交值，不立刻请求
function onSelectRole(user: UserVO, newRoleId: string) {
  if (!user.id) return
  pendingRole.value[user.id] = newRoleId
}

// 点击保存才提交修改
async function saveUserRole(user: UserVO) {
  if (!user.id) return
  const map = (pendingRole as any).value as Record<string, string | undefined>
  const newRoleId = map[user.id]
  // 无变更或与当前一致则不提交
  if (!newRoleId || newRoleId === user.role?.id) return
  savingUserId.value = user.id
  try {
    const updated = await patchUserRole({ user_id: user.id!, new_role_id: newRoleId })
    emitToast({ type: 'success', message: '角色更新成功' })
    user.role = updated.role
    // 清空该用户的待提交记录
    delete map[user.id]
  } catch (e) {
    console.error('更新角色失败', e)
    emitToast({ type: 'error', message: '更新角色失败' })
  } finally {
    savingUserId.value = null
  }
}

async function deleteUserById(userId?: string) {
  if (!userId) return
  try {
    await deleteUser(userId)
    emitToast({ type: 'success', message: '用户已删除' })
    await loadUsers()
  } catch (e) {
    console.error('删除用户失败', e)
    emitToast({ type: 'error', message: '删除用户失败' })
  }
}

function openDeleteDialog(user: UserVO) {
  userToDelete.value = user
  confirmingDelete.value = true
}

async function confirmDelete() {
  if (!userToDelete.value?.id) return
  await deleteUserById(userToDelete.value.id)
  confirmingDelete.value = false
  userToDelete.value = null
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

// 被 keep-alive 缓存后，返回该页面时自动刷新列表
onActivated(async () => {
  await loadUsers()
})
</script>

<template>
  <div class="space-y-6">
    <!-- 页面头部 -->
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h2 class="text-2xl font-bold text-foreground">用户管理</h2>
        <p class="text-sm text-muted-foreground mt-1">管理系统用户和角色权限</p>
      </div>
      <div class="flex items-center gap-3">
        <UiInput
          v-model="searchQuery"
          placeholder="搜索用户名、昵称或角色..."
          class="w-64"
        />
        <UiButton @click="router.push({ name: 'create-user' })">
          <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"></path>
          </svg>
          添加用户
        </UiButton>
      </div>
    </div>

    <!-- 用户列表卡片 -->
    <div class="bg-card rounded-lg border border-border shadow-sm">
      <!-- 表格头部 -->
      <div class="px-6 py-4 border-b border-border">
        <div class="flex items-center justify-between">
          <h3 class="text-lg font-medium text-foreground">用户列表</h3>
          <div class="text-sm text-muted-foreground">
            共 {{ filteredUsers.length }} 个用户
          </div>
        </div>
      </div>

      <!-- 表格内容 -->
      <div class="overflow-x-auto">
        <table class="w-full">
          <thead>
            <tr class="border-b border-border bg-muted/50">
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">用户信息</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">角色权限</th>
              <th class="px-6 py-4 text-left text-sm font-medium text-muted-foreground">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            <tr v-for="u in filteredUsers" :key="u.id" class="hover:bg-muted/30 transition-colors">
              <!-- 用户信息列 -->
              <td class="px-6 py-4">
                <div class="flex items-center space-x-3">
                  <div class="flex-shrink-0">
                    <div class="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
                      <svg class="w-5 h-5 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
                      </svg>
                    </div>
                  </div>
                  <div>
                    <div class="text-sm font-medium text-foreground">{{ u.username }}</div>
                    <div class="text-sm text-muted-foreground">{{ u.nickname || '未设置昵称' }}</div>
                  </div>
                </div>
              </td>

              <!-- 角色权限列 -->
              <td class="px-6 py-4">
                <div class="flex items-center gap-3">
                  <UiSelect
                    :model-value="pendingRole[u.id || ''] ?? u.role?.id"
                    class="w-48"
                    @update:modelValue="(v:any) => onSelectRole(u, v)"
                  >
                    <option v-for="r in roles" :key="r.id" :value="r.id">
                      {{ r.name }} ({{ r.role_enum }})
                    </option>
                  </UiSelect>
                  <UiButton
                    size="sm"
                    :disabled="
                      savingUserId === u.id ||
                      !pendingRole[u.id || ''] ||
                      pendingRole[u.id || ''] === u.role?.id
                    "
                    class="min-w-[60px]"
                    @click="saveUserRole(u)"
                  >
                    <span v-if="savingUserId === u.id" class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
                    <span v-else>保存</span>
                  </UiButton>
                </div>
              </td>

              <!-- 操作列 -->
              <td class="px-6 py-4">
                <div class="flex items-center gap-2">
                  <UiButton
                    size="sm"
                    variant="destructive"
                    @click="openDeleteDialog(u)"
                    class="text-xs"
                  >
                    删除用户
                  </UiButton>
                </div>
              </td>
            </tr>
          </tbody>
        </table>

        <!-- 空状态 -->
        <div v-if="filteredUsers.length === 0" class="text-center py-12">
          <svg class="w-12 h-12 text-muted-foreground mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"></path>
          </svg>
          <h3 class="text-lg font-medium text-foreground mb-2">暂无用户数据</h3>
          <p class="text-muted-foreground">{{ searchQuery ? '没有找到匹配的用户' : '还没有用户，请先添加用户' }}</p>
        </div>
      </div>
    </div>

    <!-- 分页组件 -->
    <div class="flex justify-center">
      <Pagination
        :page="pageNumber"
        :page-count="totalPages"
        :size="pageSize"
        @page-change="(p:number) => { pageNumber = p; updateQuery(); loadUsers(); }"
        @update:size="(s:number) => { pageSize = s; pageNumber = 1; updateQuery(); loadUsers(); }"
      />
    </div>

    <!-- 删除确认对话框 -->
    <UiDialog
      v-model:open="confirmingDelete"
      title="确认删除"
      @confirm="confirmDelete"
      @cancel="() => { confirmingDelete = false; userToDelete = null }"
    >
      <p class="text-sm text-muted-foreground">
        确认删除用户「{{ userToDelete?.username || '' }}」？该操作不可恢复。
      </p>
    </UiDialog>
  </div>
</template>
