<script lang="ts" setup>
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getClient, regenerateSecret, updateClient } from '../api'
import type { ClientVO } from '../api/type'
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import { emitToast } from '@/utils/toast'

const route = useRoute()
const router = useRouter()
const clientId = computed(() => Number(route.params.id))

const loading = ref(false)
const client = ref<ClientVO | null>(null)
const saving = ref(false)

// Secret
const showSecret = ref(false)
const currentSecret = ref('')
const secretCopied = ref(false)
const confirmRegenerate = ref(false)
const regenerating = ref(false)

// 编辑表单
const editForm = ref({
  name: '',
  redirect_uris: '',
})

async function loadClient() {
  loading.value = true
  try {
    client.value = await getClient(clientId.value)
    editForm.value.name = client.value.name
    editForm.value.redirect_uris = client.value.redirect_uris
  } catch (e) {
    console.error('加载应用详情失败', e)
    emitToast({ type: 'error', message: '应用不存在' })
    router.push({ name: 'oauth-list' })
  } finally {
    loading.value = false
  }
}

async function handleSave() {
  if (!client.value) return
  saving.value = true
  try {
    const updated = await updateClient(client.value.id, {
      name: editForm.value.name.trim() || undefined,
      redirect_uris: editForm.value.redirect_uris.split(',').map(s => s.trim()).filter(Boolean),
    })
    client.value = updated
    emitToast({ type: 'success', message: '已更新' })
  } catch (e) {
    console.error('更新失败', e)
  } finally {
    saving.value = false
  }
}

async function handleRegenerate() {
  if (!client.value) return
  regenerating.value = true
  try {
    const resp = await regenerateSecret(client.value.id)
    currentSecret.value = resp.client_secret
    showSecret.value = true
    secretCopied.value = false
    confirmRegenerate.value = false
    emitToast({ type: 'success', message: 'Client secret 已重新生成' })
  } catch (e) {
    console.error('生成 secret 失败', e)
  } finally {
    regenerating.value = false
  }
}

async function copySecret() {
  try {
    await navigator.clipboard.writeText(currentSecret.value)
    secretCopied.value = true
    emitToast({ type: 'success', message: '已复制到剪贴板' })
  } catch {
    emitToast({ type: 'error', message: '复制失败，请手动复制' })
  }
}

async function copyClientId() {
  if (!client.value) return
  try {
    await navigator.clipboard.writeText(client.value.client_id)
    emitToast({ type: 'success', message: 'Client ID 已复制' })
  } catch {
    emitToast({ type: 'error', message: '复制失败' })
  }
}

onMounted(async () => {
  await loadClient()
  // 从注册页跳转过来时，携带新生成的 secret
  const newSecret = route.query.new_secret as string | undefined
  if (newSecret) {
    currentSecret.value = newSecret
    showSecret.value = true
    // 清掉 query 防止刷新重复显示
    router.replace({ ...route, query: {} })
  }
})
</script>

<template>
  <div v-if="client" class="max-w-2xl space-y-6">
    <!-- 返回 -->
    <div>
      <UiButton variant="ghost" size="sm" @click="router.push({ name: 'oauth-list' })">
        <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"></path>
        </svg>
        返回应用列表
      </UiButton>
    </div>

    <!-- 标题 -->
    <div>
      <h2 class="text-2xl font-bold text-foreground">{{ client.name }}</h2>
      <p class="text-sm text-muted-foreground mt-1">应用配置和凭据管理</p>
    </div>

    <!-- Client Secret 提示框（新建后或重新生成后显示） -->
    <div v-if="showSecret" class="rounded-lg border-2 border-yellow-300 bg-yellow-50 p-4">
      <div class="flex items-start gap-3">
        <svg class="w-5 h-5 text-yellow-600 mt-0.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"></path>
        </svg>
        <div class="flex-1">
          <h4 class="text-sm font-semibold text-yellow-800">Client secret</h4>
          <p class="text-sm text-yellow-700 mt-1 mb-3">
            请立即复制 Client Secret，此后将无法再次查看。
          </p>
          <div class="flex gap-2">
            <code class="flex-1 px-3 py-2 bg-white border border-yellow-200 rounded text-sm font-mono break-all">{{ currentSecret }}</code>
            <UiButton size="sm" @click="copySecret">
              {{ secretCopied ? '已复制' : '复制' }}
            </UiButton>
          </div>
        </div>
      </div>
    </div>

    <!-- Client credentials -->
    <div class="bg-card rounded-lg border border-border shadow-sm">
      <div class="px-6 py-4 border-b border-border">
        <h3 class="text-base font-semibold text-foreground">客户端凭据</h3>
      </div>
      <div class="px-6 py-4 space-y-4">
        <!-- Client ID -->
        <div>
          <label class="block text-sm font-medium text-muted-foreground mb-1">Client ID</label>
          <div class="flex items-center gap-2">
            <code class="flex-1 px-3 py-2 bg-muted rounded text-sm font-mono">{{ client.client_id }}</code>
            <UiButton size="sm" variant="outline" @click="copyClientId">复制</UiButton>
          </div>
        </div>

        <!-- Client Secret actions -->
        <div>
          <label class="block text-sm font-medium text-muted-foreground mb-1">Client Secret</label>
          <p class="text-xs text-muted-foreground mb-2">
            需要 Client Secret 才能以应用身份进行认证。
          </p>
          <UiButton variant="outline" @click="confirmRegenerate = true">
            重新生成 Client Secret
          </UiButton>
        </div>
      </div>
    </div>

    <!-- Application settings -->
    <div class="bg-card rounded-lg border border-border shadow-sm">
      <div class="px-6 py-4 border-b border-border">
        <h3 class="text-base font-semibold text-foreground">应用设置</h3>
      </div>
      <div class="px-6 py-4 space-y-4">
        <div>
          <label class="block text-sm font-semibold text-foreground mb-1">应用名称</label>
          <UiInput v-model="editForm.name" class="w-full" />
        </div>
        <div>
          <label class="block text-sm font-semibold text-foreground mb-1">授权回调地址</label>
          <UiInput v-model="editForm.redirect_uris" class="w-full" />
          <p class="text-xs text-muted-foreground mt-1">多个地址用逗号分隔</p>
        </div>
        <div class="flex items-center gap-2">
          <label class="text-sm font-medium text-muted-foreground">允许的权限范围：</label>
          <div class="flex flex-wrap gap-1">
            <span
              v-for="s in (client.allowed_scopes || '').split(',').filter(Boolean)"
              :key="s"
              class="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-primary/10 text-primary"
            >
              {{ s.trim() }}
            </span>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <label class="text-sm font-medium text-muted-foreground">授权类型：</label>
          <div class="flex flex-wrap gap-1">
            <span
              v-for="g in (client.grant_types || '').split(',').filter(Boolean)"
              :key="g"
              class="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-muted text-muted-foreground"
            >
              {{ g.trim() }}
            </span>
          </div>
        </div>

        <div class="pt-2">
          <UiButton :disabled="saving" @click="handleSave">
            <span v-if="saving" class="mr-2 h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin inline-block"></span>
            保存更改
          </UiButton>
        </div>
      </div>
    </div>

    <!-- Regenerate secret dialog -->
    <UiDialog
      v-model:open="confirmRegenerate"
      title="生成新的 Client Secret"
      @confirm="handleRegenerate"
      @cancel="() => { confirmRegenerate = false }"
    >
      <p class="text-sm text-muted-foreground">
        生成新的 secret 后，旧的 secret 将立即失效。所有使用旧 secret 的集成都需要更新。确认继续？
      </p>
    </UiDialog>
  </div>

  <!-- Loading -->
  <div v-else-if="loading" class="flex items-center justify-center py-20">
    <span class="h-8 w-8 border-4 border-primary border-t-transparent rounded-full animate-spin"></span>
  </div>
</template>
