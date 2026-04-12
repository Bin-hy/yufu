<script lang="ts" setup>
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { createClient } from '../api'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import { emitToast } from '@/utils/toast'

const router = useRouter()
const submitting = ref(false)

const form = ref({
  name: '',
  homepage_url: '',
  description: '',
  redirect_uri: ''
})

async function handleSubmit() {
  if (!form.value.name.trim()) {
    emitToast({ type: 'error', message: '应用名称不能为空' })
    return
  }
  if (!form.value.redirect_uri.trim()) {
    emitToast({ type: 'error', message: '回调地址不能为空' })
    return
  }
  submitting.value = true
  try {
    const resp = await createClient({
      name: form.value.name.trim(),
      homepage_url: form.value.homepage_url.trim() || undefined,
      description: form.value.description.trim() || undefined,
      redirect_uri: form.value.redirect_uri.trim()
    })
    emitToast({ type: 'success', message: '应用注册成功' })
    // 跳转到详情页，携带新生成的 secret
    router.push({
      name: 'oauth-detail',
      params: { id: resp.id },
      query: { new_secret: resp.client_secret }
    })
  } catch (e) {
    console.error('注册应用失败', e)
  } finally {
    submitting.value = false
  }
}
</script>

<template>
  <div class="max-w-2xl space-y-6">
    <div>
      <h2 class="text-2xl font-bold text-foreground">注册新 OAuth 应用</h2>
    </div>

    <form class="space-y-5" @submit.prevent="handleSubmit">
      <!-- Application name -->
      <div>
        <label class="block text-sm font-semibold text-foreground mb-1">
          应用名称 <span class="text-red-500">*</span>
        </label>
        <UiInput v-model="form.name" placeholder="" class="w-full" />
        <p class="text-xs text-muted-foreground mt-1">用户能识别并信任的名称。</p>
      </div>

      <!-- Homepage URL -->
      <div>
        <label class="block text-sm font-semibold text-foreground mb-1">
          主页地址
        </label>
        <UiInput v-model="form.homepage_url" placeholder="https://example.com" class="w-full" />
        <p class="text-xs text-muted-foreground mt-1">应用主页的完整 URL。</p>
      </div>

      <!-- Application description -->
      <div>
        <label class="block text-sm font-semibold text-foreground mb-1">
          应用描述
        </label>
        <UiInput v-model="form.description" placeholder="" class="w-full" />
        <p class="text-xs text-muted-foreground mt-1">向所有用户展示的应用描述。</p>
      </div>

      <!-- Authorization callback URL -->
      <div>
        <label class="block text-sm font-semibold text-foreground mb-1">
          授权回调地址 <span class="text-red-500">*</span>
        </label>
        <UiInput v-model="form.redirect_uri" placeholder="https://example.com/callback" class="w-full" />
        <p class="text-xs text-muted-foreground mt-1">
          授权完成后用户将被重定向到此地址。
        </p>
      </div>

      <!-- Submit -->
      <div class="flex items-center gap-3 pt-2">
        <UiButton type="submit" :disabled="submitting">
          <span v-if="submitting" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
          注册应用
        </UiButton>
        <UiButton variant="outline" type="button" @click="router.back()">取消</UiButton>
      </div>
    </form>
  </div>
</template>
