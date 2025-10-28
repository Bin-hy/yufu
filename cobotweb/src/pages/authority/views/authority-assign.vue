<script setup lang="ts">
import { ref, onMounted } from 'vue';
// 暂时注释掉，避免构建错误
// import { ElMessage } from 'element-plus';
import * as authorityApi from '../api';
import type { Authority, Role, AuthorityAssignReq } from '../api/type';
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import UiButton from '@/components/ui/button/button.vue'

// 权限列表数据
const authorities = ref<Authority[]>([]);
// 角色列表数据
const roles = ref<Role[]>([]);
// 加载状态
const loading = ref(false);
// 当前选中的角色
const selectedRole = ref<Role | null>(null);
// 选中的权限ID列表
const selectedAuthorityIds = ref<string[]>([]);
// 权限分配对话框
const showAssignDialog = ref(false);

// 获取权限列表
const fetchAuthorities = async () => {
  loading.value = true;
  try {
    authorities.value = await authorityApi.getAllAuthorities();
  } catch (error) {
      console.error('获取权限列表失败');
  } finally {
    loading.value = false;
  }
};

// 获取角色列表
const fetchRoles = async () => {
  try {
    roles.value = await authorityApi.getAllRoles();
  } catch (error) {
      console.error('获取角色列表失败');
  }
};

// 打开权限分配对话框
const openAssignDialog = async (role: Role) => {
  selectedRole.value = role;
  selectedAuthorityIds.value = [];
  
  // 加载角色已有权限
  loading.value = true;
  try {
    const roleAuthorities = await authorityApi.getRoleAuthorities(role.id);
    selectedAuthorityIds.value = roleAuthorities.map(a => a.id);
    showAssignDialog.value = true;
  } catch (error) {
      console.error('获取角色权限失败');
  } finally {
    loading.value = false;
  }
};

// 保存权限分配
const saveAuthorityAssign = async () => {
  if (!selectedRole.value) return;

  loading.value = true;
  try {
    const req: AuthorityAssignReq = {
      roleId: selectedRole.value.id,
      authorityIds: selectedAuthorityIds.value
    };
    const success = await authorityApi.assignAuthorities(req);
    if (success) {
      console.log('权限分配成功');
      showAssignDialog.value = false;
    }
  } catch (error) {
    console.error('权限分配失败');
  } finally {
    loading.value = false;
  }
};

// 切换权限选择
const toggleAuthority = (authorityId: string) => {
  const index = selectedAuthorityIds.value.indexOf(authorityId);
  if (index > -1) {
    selectedAuthorityIds.value.splice(index, 1);
  } else {
    selectedAuthorityIds.value.push(authorityId);
  }
};

// 初始化
onMounted(async () => {
  await fetchAuthorities();
  await fetchRoles();
});
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
            <span class="px-2 py-1 text-xs rounded-full bg-muted text-muted-foreground">{{ role.roleEnum }}</span>
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
        <div v-else class="space-y-2">
          <div
            v-for="authority in authorities"
            :key="authority.id"
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
            <label
              :for="`authority-${authority.id}`"
              class="ml-3 flex-1 text-sm font-medium cursor-pointer"
            >
              <div class="flex justify-between items-center w-full">
                <span>{{ authority.name }}</span>
                <span class="text-xs text-muted-foreground">{{ authority.authority }}</span>
              </div>
            </label>
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