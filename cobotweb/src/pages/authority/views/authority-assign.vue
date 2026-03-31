<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { emitToast } from '@/utils/toast'
import * as authorityApi from '../api'
import * as roleApi from '@/pages/roles/api'
import type { Role, AuthRoleAddBatchReq, AuthRoleDeleteReq } from '../api/type'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import UiButton from '@/components/ui/button/button.vue'
import type { AuthorityVO } from '@/types/models'

// 权限列表数据
const authorities = ref<AuthorityVO[]>([])
// 角色列表数据
const roles = ref<Role[]>([])
// 加载状态
const loading = ref(false)
// 当前选中的角色
const selectedRole = ref<Role | null>(null)
// 选中的权限ID列表（当前勾选）
const selectedAuthorityIds = ref<number[]>([])
// 原始权限ID集合（用于 diff 计算）
const originalAuthorityIdSet = ref<Set<number>>(new Set())
// 权限分配对话框
const showAssignDialog = ref(false)

// 获取权限列表
const fetchAuthorities = async () => {
  loading.value = true
  try {
    const MAX = 1000
    const page = await authorityApi.getAuthoritiesByPage(1, MAX)
    authorities.value = page.data_list
  } catch (error) {
    console.error('获取权限列表失败', error)
    emitToast({ type: 'error', message: '获取权限列表失败' })
  } finally {
    loading.value = false
  }
}

// 获取角色列表
const fetchRoles = async () => {
  try {
    const MAX = 100
    const page = await roleApi.getRolesByPage(1, MAX)
    roles.value = page.data_list
  } catch (error) {
    console.error('获取角色列表失败', error)
    emitToast({ type: 'error', message: '获取角色列表失败' })
  }
}

// 打开权限分配对话框
const openAssignDialog = async (role: Role) => {
  selectedRole.value = role
  selectedAuthorityIds.value = []
  originalAuthorityIdSet.value = new Set()
  // 先打开对话框，随后异步拉取角色已有权限
  showAssignDialog.value = true

  // 加载角色已有权限
  loading.value = true
  try {
    const res = await authorityApi.getAuthorityByRoleId(role.id)
    const list = Array.isArray(res?.data_list) ? res.data_list : []
    const ids = list.map(a => a.id)
    selectedAuthorityIds.value = [...ids]
    originalAuthorityIdSet.value = new Set(ids)
  } catch (error) {
    console.error('获取角色权限失败', error)
    emitToast({ type: 'error', message: '获取角色权限失败' })
  } finally {
    loading.value = false
  }
}

// 基于当前勾选结果进行实时分组（已分配/未分配）
const selectedSet = computed(() => new Set(selectedAuthorityIds.value))
const assignedAuthorities = computed(() => authorities.value.filter(a => selectedSet.value.has(a.id)))
const unassignedAuthorities = computed(() => authorities.value.filter(a => !selectedSet.value.has(a.id)))

// 保存权限分配
const saveAuthorityAssign = async () => {
  if (!selectedRole.value) return

  // 计算差异：待新增、待删除
  const currentSet = new Set(selectedAuthorityIds.value)
  const originSet = originalAuthorityIdSet.value

  const toAdd: number[] = []
  const toDel: number[] = []
  for (const id of currentSet) if (!originSet.has(id)) toAdd.push(id)
  for (const id of originSet) if (!currentSet.has(id)) toDel.push(id)

  if (toAdd.length === 0 && toDel.length === 0) {
    emitToast({ type: 'info', message: '没有待提交的更改' })
    showAssignDialog.value = false
    return
  }

  loading.value = true
  try {
    let okAdd = true
    let okDel = true
    if (toAdd.length > 0) {
      const addReq: AuthRoleAddBatchReq = { role_id: selectedRole.value.id, authority_ids: toAdd }
      await authorityApi.addRoleAuthBatch(addReq)
    }
    if (toDel.length > 0) {
      const delReq: AuthRoleDeleteReq = { role_id: selectedRole.value.id, authority_ids: toDel }
      await authorityApi.deleteRoleAuthBatch(delReq)
    }
    if (okAdd && okDel) {
      emitToast({ type: 'success', message: '权限变更已保存' })
      showAssignDialog.value = false
    }
  } catch (error) {
    console.error('权限分配失败', error)
    emitToast({ type: 'error', message: '权限分配失败' })
  } finally {
    loading.value = false
  }
}

// 已使用 UiCheckbox 的 @update:modelValue 处理选中逻辑，无需额外切换函数

// 初始化
onMounted(async () => {
  await fetchAuthorities()
  await fetchRoles()
})
</script>

<template>
  <div class="rounded-lg shadow-sm overflow-hidden bg-card text-card-foreground border border-border">
    <!-- 工具栏 -->
    <div class="p-4 border-b border-border">
      <h2 class="font-semibold">角色权限分配</h2>
    </div>

    <!-- 角色列表 -->
    <div class="p-6">
      <div v-if="roles.length === 0" class="text-center py-12 text-muted-foreground">
        暂无角色数据
      </div>
      <div v-else class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <div
          v-for="role in roles"
          :key="role.id"
          class="border border-border rounded-lg p-4 hover:border-primary/30 hover:bg-primary/5 transition-colors cursor-pointer"
          @click="openAssignDialog(role)"
        >
          <div class="flex justify-between items-center mb-2">
            <h3 class="font-medium">{{ role.name }}</h3>
          </div>
          <p class="text-sm text-muted-foreground">点击为该角色分配权限</p>
        </div>
      </div>
    </div>
  </div>

  <!-- 权限分配对话框 -->
  <div v-if="showAssignDialog && selectedRole" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-card text-card-foreground rounded-lg shadow-lg max-w-2xl w-full max-h-[90vh] overflow-y-auto border border-border">
      <div class="p-4 border-b border-border flex justify-between items-center">
        <h3 class="font-semibold">为角色「{{ selectedRole.name }}」分配权限</h3>
        <UiButton variant="ghost" size="sm" aria-label="关闭" @click="showAssignDialog = false">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
          </svg>
        </UiButton>
      </div>
      <div class="p-6">
        <div v-if="authorities.length === 0" class="text-center py-8 text-muted-foreground">
          暂无可用权限
        </div>

        <div v-else class="space-y-6">
          <!-- 已分配 -->
          <div>
            <div class="flex items-center justify-between mb-2">
              <h4 class="text-sm font-semibold">已分配（{{ assignedAuthorities.length }}）</h4>
            </div>
            <div v-if="assignedAuthorities.length === 0" class="text-xs text-muted-foreground py-2">暂无</div>
            <div v-else class="space-y-2">
              <div
                v-for="authority in assignedAuthorities"
                :key="`assigned-${authority.id}`"
                class="flex items-center p-3 border border-border rounded-md hover:bg-accent hover:text-accent-foreground transition-colors"
              >
                <UiCheckbox
                  :id="`authority-${authority.id}`"
                  :modelValue="selectedAuthorityIds.includes(authority.id)"
                  @update:modelValue="(val: boolean) => {
                    const idx = selectedAuthorityIds.indexOf(authority.id)
                    if (val && idx === -1) selectedAuthorityIds.push(authority.id)
                    if (!val && idx > -1) selectedAuthorityIds.splice(idx, 1)
                  }"
                />
                <label :for="`authority-${authority.id}`" class="ml-3 flex-1 text-sm font-medium cursor-pointer">
                  <div class="flex justify-between items-center w-full">
                    <span>{{ authority.name }}</span>
                    <span class="text-xs text-muted-foreground">{{ authority.authority_uri }}</span>
                  </div>
                </label>
              </div>
            </div>
          </div>

          <!-- 未分配 -->
          <div>
            <div class="flex items-center justify-between mb-2">
              <h4 class="text-sm font-semibold">未分配（{{ unassignedAuthorities.length }}）</h4>
            </div>
            <div v-if="unassignedAuthorities.length === 0" class="text-xs text-muted-foreground py-2">暂无</div>
            <div v-else class="space-y-2">
              <div
                v-for="authority in unassignedAuthorities"
                :key="`unassigned-${authority.id}`"
                class="flex items-center p-3 border border-border rounded-md hover:bg-accent hover:text-accent-foreground transition-colors"
              >
                <UiCheckbox
                  :id="`authority-${authority.id}`"
                  :modelValue="selectedAuthorityIds.includes(authority.id)"
                  @update:modelValue="(val: boolean) => {
                    const idx = selectedAuthorityIds.indexOf(authority.id)
                    if (val && idx === -1) selectedAuthorityIds.push(authority.id)
                    if (!val && idx > -1) selectedAuthorityIds.splice(idx, 1)
                  }"
                />
                <label :for="`authority-${authority.id}`" class="ml-3 flex-1 text-sm font-medium cursor-pointer">
                  <div class="flex justify-between items-center w-full">
                    <span>{{ authority.name }}</span>
                    <span class="text-xs text-muted-foreground">{{ authority.authority_uri }}</span>
                  </div>
                </label>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div class="p-4 border-t border-border flex justify-end space-x-3">
        <UiButton variant="outline" @click="showAssignDialog = false">取消</UiButton>
        <UiButton :disabled="loading" @click="saveAuthorityAssign">{{ loading ? '分配中...' : '确定分配' }}</UiButton>
      </div>
    </div>
  </div>
</template>
