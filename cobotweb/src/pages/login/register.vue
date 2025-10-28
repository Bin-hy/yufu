<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import { showModal } from '@/utils/modal'
import { createUser } from '@/pages/users/api'

interface RegisterForm {
  username: string
  email: string
  password: string
  confirmPassword: string
  agree: boolean
}

const router = useRouter()
const form = reactive<RegisterForm>({
  username: '',
  email: '',
  password: '',
  confirmPassword: '',
  agree: false,
})

const errors = reactive({
  username: '',
  email: '',
  password: '',
  confirmPassword: '',
  agree: '',
})

const loading = ref(false)

function validateForm(): boolean {
  // 重置错误
  errors.username = ''
  errors.email = ''
  errors.password = ''
  errors.confirmPassword = ''
  errors.agree = ''

  let ok = true
  if (!form.username.trim()) { errors.username = '请输入用户名'; ok = false }
  if (form.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(form.email)) { errors.email = '邮箱格式不正确'; ok = false }
  if (!form.password) { errors.password = '请输入密码'; ok = false }
  else if (form.password.length < 6) { errors.password = '密码长度不能少于6位'; ok = false }
  if (form.confirmPassword !== form.password) { errors.confirmPassword = '两次密码输入不一致'; ok = false }
  if (!form.agree) { errors.agree = '请勾选同意用户协议与隐私政策'; ok = false }
  return ok
}

async function handleRegister() {
  if (!validateForm()) return
  try {
    loading.value = true
    // 调用后端创建用户接口（REST: POST /users）
    await createUser({
      username: form.username,
      password: form.password,
      email: form.email || undefined,
      // 可选：根据后端需求补充
      // nickname: undefined,
      // role: 'User',
      // status: 'active'
    })
    showModal({ title: '注册成功', content: '账号已创建，请使用新账号登录' })
    router.push('/login')
  } catch (err: any) {
    showModal({ title: '注册失败', content: err?.message || '请稍后重试' })
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-muted p-4">
    <div class="w-full max-w-md bg-card text-card-foreground rounded-lg shadow-xl overflow-hidden">
      <!-- 头部 -->
      <div class="bg-gradient-to-r from-primary to-secondary p-8 text-white">
        <div class="text-center mb-2">
          <h1 class="text-2xl font-bold">创建账号</h1>
          <p class="text-sm opacity-90">欢迎加入，请填写以下信息完成注册</p>
        </div>
      </div>

      <!-- 表单主体 -->
      <div class="p-6 space-y-4">
        <div>
          <label class="block text-sm font-medium mb-1">用户名</label>
          <UiInput v-model="form.username" placeholder="请输入用户名" />
          <p v-if="errors.username" class="mt-1 text-xs text-destructive">{{ errors.username }}</p>
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">邮箱（可选）</label>
          <UiInput v-model="form.email" type="email" placeholder="name@example.com" />
          <p v-if="errors.email" class="mt-1 text-xs text-destructive">{{ errors.email }}</p>
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">密码</label>
          <UiInput v-model="form.password" type="password" placeholder="请输入密码" />
          <p v-if="errors.password" class="mt-1 text-xs text-destructive">{{ errors.password }}</p>
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">确认密码</label>
          <UiInput v-model="form.confirmPassword" type="password" placeholder="再次输入密码" />
          <p v-if="errors.confirmPassword" class="mt-1 text-xs text-destructive">{{ errors.confirmPassword }}</p>
        </div>

        <div class="flex items-start gap-2">
          <UiCheckbox v-model="form.agree" />
          <div class="text-sm leading-5">
            我已阅读并同意
            <router-link to="/" class="underline">用户协议</router-link>
            与
            <router-link to="/" class="underline">隐私政策</router-link>
          </div>
        </div>
        <p v-if="errors.agree" class="-mt-2 text-xs text-destructive">{{ errors.agree }}</p>

        <UiButton class="w-full" :disabled="loading" @click="handleRegister">
          {{ loading ? '注册中...' : '注册' }}
        </UiButton>

        <div class="text-center text-sm text-muted-foreground">
          已有账号？
          <router-link to="/login" class="text-primary underline">去登录</router-link>
        </div>
      </div>
    </div>
  </div>

</template>
