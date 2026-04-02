<script lang="ts" setup>
import { ref, reactive, onMounted, computed } from 'vue'
import type { CreateUserForm, RoleOption, FormErrors } from '../api/types'
import { useRouter } from 'vue-router'
import { showModal } from '@/utils/modal'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiButton from '@/components/ui/button/button.vue'
import * as userApi from '../api'
import type { RoleVO } from '@/types/models'

// 表单类型约束
// 表单数据（严格类型）
const form = reactive<CreateUserForm>({
  username: '',
  nickname: '',
  password: '',
  confirmPassword: '',
  role_id: 0,
})

const modifiableRoles = ref<RoleVO[]>([])

const roleOptions = computed<RoleOption[]>(() => modifiableRoles.value.map(role => ({
  label: role.name || '',
  value: role.id,
})))
// 表单验证错误
const errors = reactive<FormErrors>({
  username: '',
  nickname: '',
  password: '',
  confirmPassword: ''
})

// 状态管理
const loading = ref(false)
const router = useRouter()

// 表单验证
function validateForm(): boolean {
  let isValid = true

  // 重置错误
  errors.username = ''
  errors.nickname = ''
  errors.password = ''
  errors.confirmPassword = ''

  // 用户名验证
  if (!form.username.trim()) {
    errors.username = '请输入用户名'
    isValid = false
  } else if (form.username.length < 3 || form.username.length > 20) {
    errors.username = '用户名长度应在3-20个字符之间'
    isValid = false
  }

  // 昵称验证
  if (!form.nickname.trim()) {
    errors.nickname = '请输入昵称'
    isValid = false
  } else if (form.nickname.length < 2 || form.nickname.length > 20) {
    errors.nickname = '昵称长度应在2-20个字符之间'
    isValid = false
  }

  // 密码验证
  if (!form.password) {
    errors.password = '请输入密码'
    isValid = false
  } else if (form.password.length < 6) {
    errors.password = '密码长度不能少于6位'
    isValid = false
  }

  // 确认密码验证
  if (!form.confirmPassword) {
    errors.confirmPassword = '请确认密码'
    isValid = false
  } else if (form.confirmPassword !== form.password) {
    errors.confirmPassword = '两次输入的密码不一致'
    isValid = false
  }

  return isValid
}

// 创建用户
async function handleCreate() {
  if (!validateForm()) {
    return
  }

  try {
    loading.value = true

    console.log("form.role_id type:", typeof(form.role_id))
    let res = await userApi.createUserWithRole({
      username: form.username,
      password: form.password,
      nickname: form.nickname,
      role_id: form.role_id,
    })

    // 显示成功提示并在确认后返回列表
    const ok = await showModal({
      title: '创建成功',
      content: `用户‘${res.username}’已成功创建`
    })
    if (ok) {
      router.push({ name: 'user-list' })
    }

  } catch (error: any) {
    console.error('创建用户失败:', error)
    showModal({
      title: '创建失败',
      content: error.message || '创建过程中发生错误，请稍后重试'
    })
  } finally {
    loading.value = false
  }
}

// 取消创建
function handleCancel() {
  if (Object.values(form).some(value => value)) {
    if (confirm('确定要放弃当前编辑吗？')) {
      router.push({ name: 'user-list' })
    }
  } else {
    router.push({ name: 'user-list' })
  }
}
//加载可修改的角色列表
async function loadModifiableRoles(){
  const data = await userApi.getModifiableRoles();
  modifiableRoles.value = data?.data_list
  // 角色下拉默认选中第一项（当当前值为空或不在可选项中时）
  const firstId = modifiableRoles.value?.[0]?.id ?? 0
  const exists = modifiableRoles.value?.some(r => r.id === form.role_id)
  if (!exists) {
    form.role_id = firstId
  }
}

onMounted(
  async () => {
    loadModifiableRoles();
  }
)
</script>

<template>
  <div class="rounded-lg shadow overflow-hidden bg-card text-card-foreground border border-border">
    <!-- 页面标题 -->
    <div class="px-6 py-4 border-b border-border">
      <div class="flex justify-between items-center">
        <h2 class="text-lg font-medium">创建用户</h2>
        <div>
          <UiButton variant="outline" class="mr-3" @click="handleCancel">取消</UiButton>
          <UiButton :disabled="loading" @click="handleCreate">
            <span v-if="loading" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
            创建
          </UiButton>
        </div>
      </div>
    </div>

    <!-- 创建表单 -->
    <div class="px-6 py-6">
      <form @submit.prevent="handleCreate">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
          <!-- 基本信息 -->
          <div class="space-y-4">
            <div>
              <label for="username" class="block text-sm font-medium mb-1">
                用户名 <span class="text-red-500">*</span>
              </label>
              <UiInput
                id="username"
                v-model="form.username"
                type="text"
                :class="errors.username ? 'border-destructive' : ''"
                placeholder="请输入用户名（3-20个字符）"
              />
              <p v-if="errors.username" class="mt-1 text-sm text-destructive">{{ errors.username }}</p>
            </div>

            <div>
              <label for="nickname" class="block text-sm font-medium mb-1">
                昵称 <span class="text-red-500">*</span>
              </label>
              <UiInput
                id="nickname"
                v-model="form.nickname"
                type="text"
                :class="errors.nickname ? 'border-destructive' : ''"
                placeholder="请输入昵称（2-20个字符）"
              />
              <p v-if="errors.nickname" class="mt-1 text-sm text-destructive">{{ errors.nickname }}</p>
            </div>

            <div>
              <label for="role" class="block text-sm font-medium mb-1">
                角色 <span class="text-red-500">*</span>
              </label>
              <UiSelect
                id="role"
                v-model.number="form.role_id"
                class="w-full"
              >
                <option v-for="role in roleOptions" :key="role.value" :value="role.value">{{ role.label }}</option>
              </UiSelect>
            </div>
          </div>

          <!-- 密码信息 -->
          <div class="space-y-4">
            <div>
              <label for="password" class="block text-sm font-medium mb-1">
                密码 <span class="text-red-500">*</span>
              </label>
              <UiInput
                id="password"
                v-model="form.password"
                type="password"
                :class="errors.password ? 'border-destructive' : ''"
                placeholder="请输入密码（至少6位）"
              />
              <p v-if="errors.password" class="mt-1 text-sm text-destructive">{{ errors.password }}</p>
            </div>

            <div>
              <label for="confirmPassword" class="block text-sm font-medium mb-1">
                确认密码 <span class="text-red-500">*</span>
              </label>
              <UiInput
                id="confirmPassword"
                v-model="form.confirmPassword"
                type="password"
                :class="errors.confirmPassword ? 'border-destructive' : ''"
                placeholder="请再次输入密码"
              />
              <p v-if="errors.confirmPassword" class="mt-1 text-sm text-destructive">{{ errors.confirmPassword }}</p>
            </div>

            <!-- 密码强度提示 -->
            <div v-if="form.password && !errors.password" class="bg-blue-50 p-4 rounded-md">
              <h3 class="text-sm font-medium text-blue-800 mb-2">密码强度</h3>
              <div class="w-full bg-gray-200 rounded-full h-2.5">
                <div
                  class="h-2.5 rounded-full"
                  :class="form.password.length >= 8
                    ? 'bg-green-500 w-full'
                    : form.password.length >= 6
                      ? 'bg-yellow-500 w-2/3'
                      : 'bg-red-500 w-1/3'"
                ></div>
              </div>
              <p class="mt-2 text-xs text-blue-700">
                建议使用至少8位的强密码，包含字母和数字
              </p>
            </div>
          </div>
        </div>

        <!-- 表单操作按钮（在移动设备上显示） -->
        <div class="mt-8 md:hidden flex flex-col space-y-3">
          <UiButton variant="outline" type="button" class="w-full" @click="handleCancel">取消</UiButton>
          <UiButton class="w-full" :disabled="loading" @click="handleCreate">
            <span v-if="loading" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
            创建
          </UiButton>
        </div>
      </form>
    </div>
  </div>
</template>
