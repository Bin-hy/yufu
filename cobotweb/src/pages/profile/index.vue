<script setup>
import { reactive, computed } from 'vue'
import { useUserStore } from '@/pinia/stores/user'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'

const userStore = useUserStore()
const state = reactive({
  username: userStore.userInfo?.username || '',
  nickname: userStore.userInfo?.nickname || '',
})

const initial = computed(() => ({
  username: userStore.userInfo?.username || '',
  nickname: userStore.userInfo?.nickname || '',
}))

const canSave = computed(() => state.nickname.trim() && state.nickname !== initial.value.nickname)

const save = () => {
  userStore.setUserInfo({
    ...userStore.userInfo,
    nickname: state.nickname.trim()
  })
}
</script>

<template>
  <div class="max-w-xl mx-auto p-6">
    <h1 class="text-xl font-semibold mb-4">个人中心</h1>
    <div class="rounded-lg bg-card text-card-foreground shadow">
      <div class="space-y-4 p-6">
        <div class="flex items-center gap-3">
          <div class="rounded-full w-12 h-12 bg-primary text-primary-foreground flex items-center justify-center">
            <span class="text-lg">{{ (state.nickname || state.username || '我')[0] }}</span>
          </div>
          <div>
            <div class="text-sm text-muted-foreground">用户名</div>
            <div class="font-medium">{{ state.username }}</div>
          </div>
        </div>

        <div class="space-y-1">
          <label class="text-sm">昵称</label>
          <UiInput v-model="state.nickname" type="text" placeholder="输入昵称" />
          <div class="text-xs text-muted-foreground">用于显示在导航栏和欢迎语中</div>
        </div>

        <div class="flex justify-end">
          <UiButton :disabled="!canSave" @click="save">保存</UiButton>
        </div>
      </div>
    </div>
  </div>
</template>