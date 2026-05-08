<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import { getCurrentUser, patchMyProfile } from './api'
import { emitToast } from '@/utils/toast'
import { useUserStore } from '@/pinia/stores/user'

const router = useRouter()
const userStore = useUserStore()

const loading = ref(true)
const userId = ref<number>(0)
const username = ref('')
const nickname = ref('')
const roleId = ref<number>(0)

// 修改昵称
const nicknameForm = ref({ nickname: '' })
const savingNickname = ref(false)

// 修改密码
const passwordForm = ref({ newPassword: '', confirmPassword: '' })
const savingPassword = ref(false)

onMounted(async () => {
  try {
    const info = await getCurrentUser()
    userId.value = info.id
    username.value = info.username
    nickname.value = info.nickname ?? ''
    roleId.value = info.role_id
    nicknameForm.value.nickname = info.nickname ?? ''
  } catch (e) {
    console.error('获取用户信息失败', e)
  } finally {
    loading.value = false
  }
})

async function handleSaveNickname() {
  savingNickname.value = true
  try {
    await patchMyProfile({ user_id: userId.value, nickname: nicknameForm.value.nickname.trim() || undefined })
    nickname.value = nicknameForm.value.nickname.trim()
    emitToast({ type: 'success', message: '昵称已更新' })
  } catch (e) {
    console.error('更新昵称失败', e)
  } finally {
    savingNickname.value = false
  }
}

async function handleChangePassword() {
  if (!passwordForm.value.newPassword) {
    emitToast({ type: 'error', message: '请输入新密码' })
    return
  }
  if (passwordForm.value.newPassword !== passwordForm.value.confirmPassword) {
    emitToast({ type: 'error', message: '两次输入的密码不一致' })
    return
  }
  savingPassword.value = true
  try {
    await patchMyProfile({ user_id: userId.value, password: passwordForm.value.newPassword })
    emitToast({ type: 'success', message: '密码已更新，请重新登录' })
    // 密码变更后强制退出并跳到登录页
    setTimeout(() => {
      userStore.logout()
      router.push('/login')
    }, 1200)
  } catch (e) {
    console.error('修改密码失败', e)
  } finally {
    savingPassword.value = false
  }
}
</script>

<template>
  <div class="max-w-2xl space-y-6">
    <div>
      <h2 class="text-2xl font-bold text-foreground">个人中心</h2>
      <p class="text-sm text-muted-foreground mt-1">管理你的个人信息和账户安全</p>
    </div>

    <div v-if="loading" class="flex items-center justify-center py-20">
      <span class="h-8 w-8 border-4 border-primary border-t-transparent rounded-full animate-spin"></span>
    </div>

    <template v-else>
      <!-- 个人信息 -->
      <div class="bg-card rounded-lg border border-border shadow-sm">
        <div class="px-6 py-4 border-b border-border">
          <h3 class="text-base font-semibold text-foreground">个人信息</h3>
        </div>
        <div class="px-6 py-4 space-y-4">
          <!-- 头像占位 -->
          <div class="flex items-center gap-4">
            <div class="w-16 h-16 rounded-full bg-primary text-primary-foreground flex items-center justify-center flex-shrink-0">
              <span class="text-2xl font-semibold">{{ (nickname || username || '我')[0].toUpperCase() }}</span>
            </div>
            <div>
              <div class="font-semibold text-foreground text-lg">{{ nickname || username }}</div>
              <div class="text-sm text-muted-foreground">@{{ username }}</div>
            </div>
          </div>

          <!-- 用户名（只读） -->
          <div>
            <label class="block text-sm font-medium text-muted-foreground mb-1">用户名</label>
            <div class="px-3 py-2 bg-muted rounded text-sm font-mono">{{ username }}</div>
          </div>

          <!-- 昵称（可编辑） -->
          <div>
            <label class="block text-sm font-semibold text-foreground mb-1">昵称</label>
            <UiInput v-model="nicknameForm.nickname" class="w-full" placeholder="设置昵称" />
          </div>

          <div>
            <UiButton :disabled="savingNickname" @click="handleSaveNickname">
              <span v-if="savingNickname" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
              保存昵称
            </UiButton>
          </div>
        </div>
      </div>

      <!-- 修改密码 -->
      <div class="bg-card rounded-lg border border-border shadow-sm">
        <div class="px-6 py-4 border-b border-border">
          <h3 class="text-base font-semibold text-foreground">修改密码</h3>
        </div>
        <div class="px-6 py-4 space-y-4">
          <div>
            <label class="block text-sm font-semibold text-foreground mb-1">新密码</label>
            <UiInput v-model="passwordForm.newPassword" type="password" class="w-full" placeholder="输入新密码" />
          </div>
          <div>
            <label class="block text-sm font-semibold text-foreground mb-1">确认新密码</label>
            <UiInput v-model="passwordForm.confirmPassword" type="password" class="w-full" placeholder="再次输入新密码" />
          </div>
          <div>
            <UiButton :disabled="savingPassword" @click="handleChangePassword">
              <span v-if="savingPassword" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
              修改密码
            </UiButton>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
