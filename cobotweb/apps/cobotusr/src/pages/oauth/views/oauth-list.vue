<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { listClients, deleteClient } from '../api'
import type { ClientVO } from '../api/type'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import { emitToast } from '@/utils/toast'

const router = useRouter()
const loading = ref(false)
const clients = ref<ClientVO[]>([])

const confirmingDelete = ref(false)
const clientToDelete = ref<ClientVO | null>(null)

async function loadClients() {
  loading.value = true
  try {
    clients.value = await listClients()
  } catch (e) {
    console.error('加载应用列表失败', e)
  } finally {
    loading.value = false
  }
}

function openDeleteDialog(client: ClientVO) {
  clientToDelete.value = client
  confirmingDelete.value = true
}

async function confirmDelete() {
  if (!clientToDelete.value) return
  try {
    await deleteClient(clientToDelete.value.id)
    emitToast({ type: 'success', message: '应用已删除' })
    await loadClients()
  } catch (e) {
    console.error('删除失败', e)
    emitToast({ type: 'error', message: '删除失败' })
  } finally {
    confirmingDelete.value = false
    clientToDelete.value = null
  }
}

onMounted(loadClients)
</script>

<template>
  <div class="space-y-6">
    <!-- 页面头部 -->
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h2 class="text-2xl font-bold text-foreground">OAuth 应用</h2>
        <p class="text-sm text-muted-foreground mt-1">管理已注册的 OAuth 应用，供第三方接入使用</p>
      </div>
      <UiButton @click="router.push({ name: 'oauth-create' })">
        <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"></path>
        </svg>
        注册新应用
      </UiButton>
    </div>

    <!-- 应用列表 -->
    <div class="space-y-3">
      <div
        v-for="c in clients"
        :key="c.id"
        class="bg-card rounded-lg border border-border shadow-sm hover:border-primary/50 transition-colors cursor-pointer"
        @click="router.push({ name: 'oauth-detail', params: { id: c.id } })"
      >
        <div class="px-6 py-4 flex items-center justify-between">
          <div class="flex items-center gap-4">
            <!-- App Icon -->
            <div class="w-12 h-12 rounded-lg bg-muted flex items-center justify-center flex-shrink-0">
              <svg class="w-6 h-6 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
              </svg>
            </div>
            <div>
              <div class="text-base font-semibold text-foreground">{{ c.name }}</div>
              <div class="text-sm text-muted-foreground mt-0.5">
                Client ID: <code class="text-xs bg-muted px-1.5 py-0.5 rounded">{{ c.client_id }}</code>
              </div>
            </div>
          </div>
          <div class="flex items-center gap-4">
            <div class="text-xs text-muted-foreground hidden sm:block">
              {{ c.create_time }}
            </div>
            <UiButton
              size="sm"
              variant="destructive"
              @click.stop="openDeleteDialog(c)"
            >
              删除
            </UiButton>
          </div>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-if="clients.length === 0 && !loading" class="bg-card rounded-lg border border-border shadow-sm text-center py-16">
        <svg class="w-16 h-16 text-muted-foreground mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
        </svg>
        <h3 class="text-lg font-medium text-foreground mb-2">还没有 OAuth 应用</h3>
        <p class="text-muted-foreground mb-4">注册一个 OAuth 应用以允许第三方接入</p>
        <UiButton @click="router.push({ name: 'oauth-create' })">注册新应用</UiButton>
      </div>
    </div>

    <!-- 删除确认 -->
    <UiDialog
      v-model:open="confirmingDelete"
      title="确认删除"
      @confirm="confirmDelete"
      @cancel="() => { confirmingDelete = false; clientToDelete = null }"
    >
      <p class="text-sm text-muted-foreground">
        确认删除应用「<strong>{{ clientToDelete?.name }}</strong>」？所有使用此应用的 token 将失效，且该操作不可恢复。
      </p>
    </UiDialog>
  </div>
</template>
