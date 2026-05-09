<script lang="ts" setup>
import { ref, computed, onMounted, onActivated } from 'vue'
import { emitToast } from '@/utils/toast'
import { getUsersByPage, getModifiableRoles, patchUserRole, deleteUser, getUserAuthorityOverridesByPage, addUserAuthorityOverride, deleteUserAuthorityOverride, patchUserAuthorityOverride, getUserPermissionOverridesByPage, addUserPermissionOverride, patchUserPermissionOverride, deleteUserPermissionOverride } from '../api'
import type { RoleVO, UserVO } from '@/types/models'
import type { AuthorityVO, UserAuthorityVO, PermissionVO, UserPermVO } from '@/types/models'
import Pagination from '@/components/ui/pagination/pagination.vue'
import { useRoute, useRouter } from 'vue-router'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import * as authorityApi from '@/pages/authority/api'
import * as permApi from '@/pages/permissions/api'
import { kickoutUserRequest } from '@/pages/login/api'

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
const savingUserId = ref<number | null>(null)

// 待提交的角色选择（键为用户ID，值为角色ID）
const pendingRole = ref<Record<number, number | undefined>>({})

// 删除确认弹窗状态
const confirmingDelete = ref(false)
const userToDelete = ref<UserVO | null>(null)
// 权限覆写弹窗
const overrideDialogOpen = ref(false)
const overrideUser = ref<UserVO | null>(null)
const overrideLoading = ref(false)
const overrideTab = ref<'authority' | 'permission'>('authority')

// --- authority tab state ---
const overrides = ref<UserAuthorityVO[]>([])
const authOptions = ref<AuthorityVO[]>([])
const roleAuthorityIds = ref<Set<number>>(new Set())
const authQuery = ref('')
const selectedAuthorityId = ref<number | null>(null)
const selectedAllow = ref<boolean>(true)

const filteredAuthOptions = computed(() => {
  const q = authQuery.value.trim().toLowerCase()
  if (!q) return authOptions.value
  return authOptions.value.filter(a =>
    a.name.toLowerCase().includes(q) || a.authority_uri.toLowerCase().includes(q)
  )
})

// --- permission tab state ---
const permOptions = ref<PermissionVO[]>([])
const permOverrides = ref<UserPermVO[]>([])
const rolePermIds = ref<Set<number>>(new Set())
const permQuery = ref('')
const selectedPermId = ref<number | null>(null)
const selectedPermAllow = ref<boolean>(true)

const filteredPermOptions = computed(() => {
  const q = permQuery.value.trim().toLowerCase()
  if (!q) return permOptions.value
  return permOptions.value.filter(p =>
    p.name.toLowerCase().includes(q) || p.action.toLowerCase().includes(q)
  )
})

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
function onSelectRole(user: UserVO, newRoleId: number) {
  if (!user.id) return
  pendingRole.value[user.id] = newRoleId
}

// 点击保存才提交修改
async function saveUserRole(user: UserVO) {
  if (!user.id) return
  const map = pendingRole.value as Record<number, number | undefined>
  const newRoleId = map[user.id]
  // 无变更或与当前一致则不提交
  if (newRoleId == null || newRoleId === user.role?.id) return
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

async function deleteUserById(userId?: number) {
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

// 踢下线
async function kickoutUser(user: UserVO) {
  if (!user?.id) return
  try {
    await kickoutUserRequest({ sub: user.id })
    emitToast({ type: 'success', message: '已将用户踢下线' })
  } catch (e) {
    console.error('踢下线失败', e)
    emitToast({ type: 'error', message: '踢下线失败' })
  }
}

function openDeleteDialog(user: UserVO) {
  userToDelete.value = user
  confirmingDelete.value = true
}

async function openOverrideDialog(user: UserVO) {
  overrideUser.value = user
  overrideTab.value = 'authority'
  overrideDialogOpen.value = true
  overrideLoading.value = true
  try {
    // Load authority data
    const authPage = await authorityApi.getAuthoritiesByPage(1, 1000)
    authOptions.value = authPage.data_list || []
    const rolePage = await authorityApi.getAuthorityByRoleId(user.role_id)
    roleAuthorityIds.value = new Set((rolePage.data_list || []).map((a: AuthorityVO) => a.id))
    const ovPage = await getUserAuthorityOverridesByPage(1, 1000)
    overrides.value = (ovPage.data_list || []).filter((i: UserAuthorityVO) => i.user_vo?.id === user.id)

    // Load permission data
    const permPage = await permApi.getPermissionsByPage(1, 1000)
    permOptions.value = permPage.data_list || []
    const rolePermPage = await permApi.getRolePermissionsByRoleId(user.role_id)
    rolePermIds.value = new Set((rolePermPage.data_list || []).map((p: PermissionVO) => p.id))
    const povPage = await getUserPermissionOverridesByPage(1, 1000)
    permOverrides.value = (povPage.data_list || []).filter((i: UserPermVO) => i.user_vo?.id === user.id)
  } catch (e) {
    console.error('加载权限覆写数据失败', e)
  } finally {
    overrideLoading.value = false
  }
}

async function addOverride() {
  if (!overrideUser.value?.id || selectedAuthorityId.value == null) return
  try {
    overrideLoading.value = true
    await addUserAuthorityOverride({
      user_id: overrideUser.value.id,
      authority_id: selectedAuthorityId.value,
      allow: selectedAllow.value,
    })
    // 刷新覆写列表
    const ovPage = await getUserAuthorityOverridesByPage(1, 1000)
    overrides.value = (ovPage.data_list || []).filter(i => i.user_vo?.id === overrideUser.value!.id)
    // 清空选择
    selectedAuthorityId.value = null
    selectedAllow.value = true
  } catch (e) {
    console.error('新增权限覆写失败', e)
  } finally {
    overrideLoading.value = false
  }
}

// 取消：旧的覆写切换/删除函数，改为统一的 setOverrideAllow / clearOverride

// 设置或更新指定权限的覆写为 allow=true/false
async function setOverrideAllow(authorityId: number, allow: boolean) {
  if (!overrideUser.value?.id) return
  const existing = overrides.value.find(i => i.authority_vo?.id === authorityId)
  try {
    overrideLoading.value = true
    if (existing) {
      await patchUserAuthorityOverride({
        user_id: overrideUser.value.id,
        authority_id: authorityId,
        allow,
      })
    } else {
      await addUserAuthorityOverride({
        user_id: overrideUser.value.id,
        authority_id: authorityId,
        allow,
      })
    }
    const ovPage = await getUserAuthorityOverridesByPage(1, 1000)
    overrides.value = (ovPage.data_list || []).filter(i => i.user_vo?.id === overrideUser.value!.id)
  } catch (e) {
    console.error('设置覆写失败', e)
  } finally {
    overrideLoading.value = false
  }
}

// 取消覆写（删除）
async function clearOverride(authorityId: number) {
  if (!overrideUser.value?.id) return
  const existing = overrides.value.find(i => i.authority_vo?.id === authorityId)
  if (!existing) return
  try {
    overrideLoading.value = true
    await deleteUserAuthorityOverride({
      user_id: overrideUser.value.id,
      authority_id: authorityId,
      allow: existing.allow,
    })
    const ovPage = await getUserAuthorityOverridesByPage(1, 1000)
    overrides.value = (ovPage.data_list || []).filter(i => i.user_vo?.id === overrideUser.value!.id)
  } catch (e) {
    console.error('取消覆写失败', e)
  } finally {
    overrideLoading.value = false
  }
}

// --- Permission overrides ---
async function addPermOverride() {
  if (!overrideUser.value?.id || selectedPermId.value == null) return
  try {
    overrideLoading.value = true
    await addUserPermissionOverride({
      user_id: overrideUser.value.id,
      permission_id: selectedPermId.value,
      allow: selectedPermAllow.value,
    })
    const povPage = await getUserPermissionOverridesByPage(1, 1000)
    permOverrides.value = (povPage.data_list || []).filter((i: UserPermVO) => i.user_vo?.id === overrideUser.value!.id)
    selectedPermId.value = null
    selectedPermAllow.value = true
  } catch (e) {
    console.error('新增资源权限覆写失败', e)
  } finally {
    overrideLoading.value = false
  }
}

async function setPermOverrideAllow(permId: number, allow: boolean) {
  if (!overrideUser.value?.id) return
  const existing = permOverrides.value.find(i => i.permission_vo?.id === permId)
  try {
    overrideLoading.value = true
    if (existing) {
      await patchUserPermissionOverride({
        user_id: overrideUser.value.id,
        permission_id: permId,
        allow,
      })
    } else {
      await addUserPermissionOverride({
        user_id: overrideUser.value.id,
        permission_id: permId,
        allow,
      })
    }
    const povPage = await getUserPermissionOverridesByPage(1, 1000)
    permOverrides.value = (povPage.data_list || []).filter((i: UserPermVO) => i.user_vo?.id === overrideUser.value!.id)
  } catch (e) {
    console.error('设置资源权限覆写失败', e)
  } finally {
    overrideLoading.value = false
  }
}

async function clearPermOverride(permId: number) {
  if (!overrideUser.value?.id) return
  const existing = permOverrides.value.find(i => i.permission_vo?.id === permId)
  if (!existing) return
  try {
    overrideLoading.value = true
    await deleteUserPermissionOverride({
      user_id: overrideUser.value.id,
      permission_id: permId,
    })
    const povPage = await getUserPermissionOverridesByPage(1, 1000)
    permOverrides.value = (povPage.data_list || []).filter((i: UserPermVO) => i.user_vo?.id === overrideUser.value!.id)
  } catch (e) {
    console.error('取消资源权限覆写失败', e)
  } finally {
    overrideLoading.value = false
  }
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
                    :model-value="pendingRole[u.id] ?? u.role?.id"
                    class="w-48"
                    @update:modelValue="(v:any) => onSelectRole(u, Number(v))"
                  >
                    <option v-for="r in roles" :key="r.id" :value="r.id">
                      {{ r.name }}
                    </option>
                  </UiSelect>
                  <UiButton
                    size="sm"
                    :disabled="
                      savingUserId === u.id ||
                      pendingRole[u.id] == null ||
                      pendingRole[u.id] === u.role?.id
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
                  <UiButton
                    size="sm"
                    variant="outline"
                    class="text-xs"
                    @click="openOverrideDialog(u)"
                  >
                    权限覆写
                  </UiButton>
                  <UiButton
                    size="sm"
                    variant="outline"
                    class="text-xs"
                    @click="kickoutUser(u)"
                  >
                    踢下线
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

    <!-- 权限覆写设置弹窗 -->
    <UiDialog
      v-model:open="overrideDialogOpen"
      :title="overrideUser ? `权限覆写 - ${overrideUser.username}` : '权限覆写'"
      size="xl"
      @cancel="() => { overrideDialogOpen = false; overrideUser = null }"
      @confirm="() => {}"
    >
      <div class="space-y-4">
        <!-- Tab 切换 -->
        <div class="flex border-b border-border">
          <button
            class="px-4 py-2 text-sm font-medium border-b-2 transition-colors"
            :class="overrideTab === 'authority' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
            @click="overrideTab = 'authority'"
          >API 权限</button>
          <button
            class="px-4 py-2 text-sm font-medium border-b-2 transition-colors"
            :class="overrideTab === 'permission' ? 'border-primary text-primary' : 'border-transparent text-muted-foreground hover:text-foreground'"
            @click="overrideTab = 'permission'"
          >资源权限</button>
        </div>

        <!-- Authority Tab -->
        <template v-if="overrideTab === 'authority'">
          <div class="rounded-md border border-border p-3">
            <div class="flex items-end gap-3">
              <div class="flex-1">
                <label class="block text-xs font-medium mb-1">搜索权限</label>
                <UiInput v-model="authQuery" placeholder="按名称或URI搜索..." />
              </div>
              <div class="flex-1">
                <label class="block text-xs font-medium mb-1">选择权限</label>
                <UiSelect v-model.number="selectedAuthorityId" class="w-full">
                  <option :value="null">请选择</option>
                  <option v-for="a in filteredAuthOptions" :key="a.id" :value="a.id">{{ a.name }}（{{ a.authority_uri }}）</option>
                </UiSelect>
              </div>
              <div>
                <label class="block text-xs font-medium mb-1">是否允许</label>
                <div class="flex items-center h-9">
                  <UiCheckbox v-model="selectedAllow" />
                  <span class="ml-2 text-sm">允许</span>
                </div>
              </div>
              <UiButton :disabled="overrideLoading || selectedAuthorityId == null" @click="addOverride">
                <span v-if="overrideLoading" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
                添加覆写
              </UiButton>
            </div>
          </div>

          <div class="overflow-x-auto">
            <table class="min-w-full divide-y divide-border text-sm">
              <thead>
                <tr>
                  <th class="px-3 py-2 text-left">权限名称</th>
                  <th class="px-3 py-2 text-left">权限标识</th>
                  <th class="px-3 py-2 text-left">角色拥有</th>
                  <th class="px-3 py-2 text-left">覆写</th>
                  <th class="px-3 py-2 text-left">实际权限</th>
                  <th class="px-3 py-2 text-left">操作</th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="overrideLoading"><td colspan="6" class="px-3 py-6 text-center text-muted-foreground">加载中...</td></tr>
                <tr v-else-if="filteredAuthOptions.length === 0"><td colspan="6" class="px-3 py-6 text-center text-muted-foreground">暂无权限数据</td></tr>
                <tr v-else v-for="a in filteredAuthOptions" :key="a.id" class="border-t border-border">
                  <td class="px-3 py-2">{{ a.name }}</td>
                  <td class="px-3 py-2">{{ a.authority_uri }}</td>
                  <td class="px-3 py-2">
                    <span :class="roleAuthorityIds.has(a.id) ? 'text-green-600' : 'text-muted-foreground'">{{ roleAuthorityIds.has(a.id) ? '是' : '否' }}</span>
                  </td>
                  <td class="px-3 py-2">
                    <template v-if="overrides.find(i => i.authority_vo?.id === a.id)">
                      <span :class="(overrides.find(i => i.authority_vo?.id === a.id)!.allow) ? 'text-green-600' : 'text-red-600'">
                        {{ (overrides.find(i => i.authority_vo?.id === a.id)!.allow) ? '允许' : '拒绝' }}
                      </span>
                    </template>
                    <template v-else><span class="text-muted-foreground">未设置</span></template>
                  </td>
                  <td class="px-3 py-2">
                    <span :class="((overrides.find(i => i.authority_vo?.id === a.id)?.allow) ?? roleAuthorityIds.has(a.id)) ? 'text-green-600' : 'text-red-600'">
                      {{ ((overrides.find(i => i.authority_vo?.id === a.id)?.allow) ?? roleAuthorityIds.has(a.id)) ? '有权限' : '无权限' }}
                    </span>
                  </td>
                  <td class="px-3 py-2 flex gap-2">
                    <UiButton size="sm" variant="outline" @click="() => setOverrideAllow(a.id, true)">允许</UiButton>
                    <UiButton size="sm" variant="outline" @click="() => setOverrideAllow(a.id, false)">拒绝</UiButton>
                    <UiButton size="sm" variant="destructive" @click="() => clearOverride(a.id)" :disabled="!overrides.find(i => i.authority_vo?.id === a.id)">取消覆写</UiButton>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>

        <!-- Permission Tab -->
        <template v-if="overrideTab === 'permission'">
          <div class="rounded-md border border-border p-3">
            <div class="flex items-end gap-3">
              <div class="flex-1">
                <label class="block text-xs font-medium mb-1">搜索权限</label>
                <UiInput v-model="permQuery" placeholder="按名称或动作搜索..." />
              </div>
              <div class="flex-1">
                <label class="block text-xs font-medium mb-1">选择权限</label>
                <UiSelect v-model.number="selectedPermId" class="w-full">
                  <option :value="null">请选择</option>
                  <option v-for="p in filteredPermOptions" :key="p.id" :value="p.id">{{ p.name }}（{{ p.action }} {{ p.resource_type || '*' }}）</option>
                </UiSelect>
              </div>
              <div>
                <label class="block text-xs font-medium mb-1">是否允许</label>
                <div class="flex items-center h-9">
                  <UiCheckbox v-model="selectedPermAllow" />
                  <span class="ml-2 text-sm">允许</span>
                </div>
              </div>
              <UiButton :disabled="overrideLoading || selectedPermId == null" @click="addPermOverride">
                <span v-if="overrideLoading" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
                添加覆写
              </UiButton>
            </div>
          </div>

          <div class="overflow-x-auto">
            <table class="min-w-full divide-y divide-border text-sm">
              <thead>
                <tr>
                  <th class="px-3 py-2 text-left">权限名</th>
                  <th class="px-3 py-2 text-left">动作</th>
                  <th class="px-3 py-2 text-left">资源</th>
                  <th class="px-3 py-2 text-left">角色拥有</th>
                  <th class="px-3 py-2 text-left">覆写</th>
                  <th class="px-3 py-2 text-left">实际权限</th>
                  <th class="px-3 py-2 text-left">操作</th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="overrideLoading"><td colspan="7" class="px-3 py-6 text-center text-muted-foreground">加载中...</td></tr>
                <tr v-else-if="filteredPermOptions.length === 0"><td colspan="7" class="px-3 py-6 text-center text-muted-foreground">暂无权限数据</td></tr>
                <tr v-else v-for="p in filteredPermOptions" :key="p.id" class="border-t border-border">
                  <td class="px-3 py-2">
                    <span class="font-mono text-primary bg-primary/10 px-1.5 py-0.5 rounded text-xs">{{ p.name }}</span>
                  </td>
                  <td class="px-3 py-2">{{ p.action }}</td>
                  <td class="px-3 py-2 text-xs text-muted-foreground">{{ p.resource_type || '*' }}:{{ p.resource_pattern || '*' }}</td>
                  <td class="px-3 py-2">
                    <span :class="rolePermIds.has(p.id) ? 'text-green-600' : 'text-muted-foreground'">{{ rolePermIds.has(p.id) ? '是' : '否' }}</span>
                  </td>
                  <td class="px-3 py-2">
                    <template v-if="permOverrides.find(i => i.permission_vo?.id === p.id)">
                      <span :class="permOverrides.find(i => i.permission_vo?.id === p.id)!.allow ? 'text-green-600' : 'text-red-600'">
                        {{ permOverrides.find(i => i.permission_vo?.id === p.id)!.allow ? '允许' : '拒绝' }}
                      </span>
                    </template>
                    <template v-else><span class="text-muted-foreground">未设置</span></template>
                  </td>
                  <td class="px-3 py-2">
                    <span :class="((permOverrides.find(i => i.permission_vo?.id === p.id)?.allow) ?? rolePermIds.has(p.id)) ? 'text-green-600' : 'text-red-600'">
                      {{ ((permOverrides.find(i => i.permission_vo?.id === p.id)?.allow) ?? rolePermIds.has(p.id)) ? '有权限' : '无权限' }}
                    </span>
                  </td>
                  <td class="px-3 py-2 flex gap-2">
                    <UiButton size="sm" variant="outline" @click="() => setPermOverrideAllow(p.id, true)">允许</UiButton>
                    <UiButton size="sm" variant="outline" @click="() => setPermOverrideAllow(p.id, false)">拒绝</UiButton>
                    <UiButton size="sm" variant="destructive" @click="() => clearPermOverride(p.id)" :disabled="!permOverrides.find(i => i.permission_vo?.id === p.id)">取消覆写</UiButton>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
      </div>
    </UiDialog>
  </div>
</template>
