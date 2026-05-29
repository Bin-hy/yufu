<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { emitToast } from '@/utils/toast'
import * as permApi from '../api'
import * as roleApi from '@/pages/roles/api'
import type { RoleVO, PermissionVO } from '@/types/models'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import UiButton from '@/components/ui/button/button.vue'

const permissions = ref<PermissionVO[]>([])
const roles = ref<RoleVO[]>([])
const loading = ref(false)
const selectedRole = ref<RoleVO | null>(null)
const selectedPermissionIds = ref<number[]>([])
const originalPermissionIdSet = ref<Set<number>>(new Set())
const showAssignDialog = ref(false)

async function fetchPermissions() {
  loading.value = true
  try {
    const page = await permApi.getPermissionsByPage(1, 1000)
    permissions.value = page.data_list
  } catch (error) {
    console.error('获取权限列表失败', error)
    emitToast({ type: 'error', message: '获取权限列表失败' })
  } finally {
    loading.value = false
  }
}

async function fetchRoles() {
  try {
    const page = await roleApi.getRolesByPage(1, 100)
    roles.value = page.data_list
  } catch (error) {
    console.error('获取角色列表失败', error)
    emitToast({ type: 'error', message: '获取角色列表失败' })
  }
}

async function openAssignDialog(role: RoleVO) {
  selectedRole.value = role
  selectedPermissionIds.value = []
  originalPermissionIdSet.value = new Set()
  showAssignDialog.value = true

  loading.value = true
  try {
    const res = await permApi.getRolePermissionsByRoleId(role.id)
    const list = Array.isArray(res?.data_list) ? res.data_list : []
    const ids = list.map((p: PermissionVO) => p.id)
    selectedPermissionIds.value = [...ids]
    originalPermissionIdSet.value = new Set(ids)
  } catch (error) {
    console.error('获取角色权限失败', error)
    emitToast({ type: 'error', message: '获取角色权限失败' })
  } finally {
    loading.value = false
  }
}

const selectedSet = computed(() => new Set(selectedPermissionIds.value))
const assignedPermissions = computed(() =>
  permissions.value.filter(p => selectedSet.value.has(p.id))
)
const unassignedPermissions = computed(() =>
  permissions.value.filter(p => !selectedSet.value.has(p.id))
)

async function saveAssign() {
  if (!selectedRole.value) return

  const currentSet = new Set(selectedPermissionIds.value)
  const originSet = originalPermissionIdSet.value

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
    if (toAdd.length > 0) {
      await permApi.addRolePermissionBatch({ role_id: selectedRole.value.id, permission_ids: toAdd })
    }
    if (toDel.length > 0) {
      await permApi.deleteRolePermissionBatch({ role_id: selectedRole.value.id, permission_ids: toDel })
    }
    emitToast({ type: 'success', message: '权限分配已保存' })
    showAssignDialog.value = false
  } catch (error) {
    console.error('权限分配失败', error)
    emitToast({ type: 'error', message: '权限分配失败' })
  } finally {
    loading.value = false
  }
}

onMounted(async () => {
  await fetchPermissions()
  await fetchRoles()
})
</script>

<template>
  <div class="rounded-lg shadow-sm overflow-hidden bg-card text-card-foreground border border-border">
    <div class="p-4 border-b border-border">
      <h2 class="font-semibold">角色权限分配</h2>
    </div>

    <div class="p-6">
      <div v-if="roles.length === 0" class="text-center py-12 text-muted-foreground">暂无角色数据</div>
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
          <p class="text-sm text-muted-foreground">点击为该角色分配资源权限</p>
        </div>
      </div>
    </div>
  </div>

  <!-- 分配对话框 -->
  <div v-if="showAssignDialog && selectedRole" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-card text-card-foreground rounded-lg shadow-lg max-w-2xl w-full max-h-[90vh] overflow-y-auto border border-border">
      <div class="p-4 border-b border-border flex justify-between items-center">
        <h3 class="font-semibold">为角色「{{ selectedRole.name }}」分配资源权限</h3>
        <UiButton variant="ghost" size="sm" aria-label="关闭" @click="showAssignDialog = false">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
          </svg>
        </UiButton>
      </div>
      <div class="p-6">
        <div v-if="permissions.length === 0" class="text-center py-8 text-muted-foreground">暂无可用权限</div>

        <div v-else class="space-y-6">
          <div>
            <div class="flex items-center justify-between mb-2">
              <h4 class="text-sm font-semibold">已分配（{{ assignedPermissions.length }}）</h4>
            </div>
            <div v-if="assignedPermissions.length === 0" class="text-xs text-muted-foreground py-2">暂无</div>
            <div v-else class="space-y-2">
              <div
                v-for="perm in assignedPermissions"
                :key="`assigned-${perm.id}`"
                class="flex items-center p-3 border border-border rounded-md hover:bg-accent hover:text-accent-foreground transition-colors"
              >
                <UiCheckbox
                  :id="`perm-${perm.id}`"
                  :modelValue="selectedPermissionIds.includes(perm.id)"
                  @update:modelValue="(val: boolean) => {
                    const idx = selectedPermissionIds.indexOf(perm.id)
                    if (val && idx === -1) selectedPermissionIds.push(perm.id)
                    if (!val && idx > -1) selectedPermissionIds.splice(idx, 1)
                  }"
                />
                <label :for="`perm-${perm.id}`" class="ml-3 flex-1 text-sm font-medium cursor-pointer">
                  <div class="flex justify-between items-center w-full">
                    <span>
                      <span class="font-mono text-primary bg-primary/10 px-1.5 py-0.5 rounded text-xs mr-2">{{ perm.name }}</span>
                      {{ perm.action }}
                    </span>
                    <span class="text-xs text-muted-foreground">{{ perm.resource_type || '*' }}:{{ perm.resource_pattern || '*' }}</span>
                  </div>
                </label>
              </div>
            </div>
          </div>

          <div>
            <div class="flex items-center justify-between mb-2">
              <h4 class="text-sm font-semibold">未分配（{{ unassignedPermissions.length }}）</h4>
            </div>
            <div v-if="unassignedPermissions.length === 0" class="text-xs text-muted-foreground py-2">暂无</div>
            <div v-else class="space-y-2">
              <div
                v-for="perm in unassignedPermissions"
                :key="`unassigned-${perm.id}`"
                class="flex items-center p-3 border border-border rounded-md hover:bg-accent hover:text-accent-foreground transition-colors"
              >
                <UiCheckbox
                  :id="`perm-${perm.id}`"
                  :modelValue="selectedPermissionIds.includes(perm.id)"
                  @update:modelValue="(val: boolean) => {
                    const idx = selectedPermissionIds.indexOf(perm.id)
                    if (val && idx === -1) selectedPermissionIds.push(perm.id)
                    if (!val && idx > -1) selectedPermissionIds.splice(idx, 1)
                  }"
                />
                <label :for="`perm-${perm.id}`" class="ml-3 flex-1 text-sm font-medium cursor-pointer">
                  <div class="flex justify-between items-center w-full">
                    <span>
                      <span class="font-mono text-primary bg-primary/10 px-1.5 py-0.5 rounded text-xs mr-2">{{ perm.name }}</span>
                      {{ perm.action }}
                    </span>
                    <span class="text-xs text-muted-foreground">{{ perm.resource_type || '*' }}:{{ perm.resource_pattern || '*' }}</span>
                  </div>
                </label>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div class="p-4 border-t border-border flex justify-end space-x-3">
        <UiButton variant="outline" @click="showAssignDialog = false">取消</UiButton>
        <UiButton :disabled="loading" @click="saveAssign">{{ loading ? '分配中...' : '确定分配' }}</UiButton>
      </div>
    </div>
  </div>
</template>
