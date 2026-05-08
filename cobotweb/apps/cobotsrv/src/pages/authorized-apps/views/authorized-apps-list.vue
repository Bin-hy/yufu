<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { listAuthorizedApps, revokeAuthorizedApp } from '../api'
import type { AuthorizedAppVO } from '../api/type'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import { emitToast } from '@/utils/toast'

const loading = ref(false)
const apps = ref<AuthorizedAppVO[]>([])

const confirmingRevoke = ref(false)
const appToRevoke = ref<AuthorizedAppVO | null>(null)

async function loadApps() {
  loading.value = true
  try {
    apps.value = await listAuthorizedApps()
  } catch (e) {
    console.error('加载已授权应用失败', e)
  } finally {
    loading.value = false
  }
}

function openRevokeDialog(app: AuthorizedAppVO) {
  appToRevoke.value = app
  confirmingRevoke.value = true
}

async function confirmRevoke() {
  if (!appToRevoke.value) return
  try {
    await revokeAuthorizedApp(appToRevoke.value.consent_id)
    emitToast({ type: 'success', message: '已撤销授权' })
    await loadApps()
  } catch (e) {
    console.error('撤销失败', e)
    emitToast({ type: 'error', message: '撤销失败' })
  } finally {
    confirmingRevoke.value = false
    appToRevoke.value = null
  }
}

onMounted(loadApps)
</script>

<template>
  <div class="space-y-6">
    <!-- 页面头部 -->
    <div>
      <h2 class="text-2xl font-bold text-foreground">已授权应用</h2>
      <p class="text-sm text-muted-foreground mt-1">
        管理你已授权访问账户的第三方应用。撤销授权后，该应用将无法再访问你的数据。
      </p>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="flex items-center justify-center py-20">
      <span class="h-8 w-8 border-4 border-primary border-t-transparent rounded-full animate-spin"></span>
    </div>

    <!-- 应用列表 -->
    <div v-else class="space-y-3">
      <div
        v-for="app in apps"
        :key="app.consent_id"
        class="bg-card rounded-lg border border-border shadow-sm"
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
              <div class="text-base font-semibold text-foreground">{{ app.client_name }}</div>
              <div class="text-sm text-muted-foreground mt-0.5">
                授权时间：{{ app.authorized_at }}
              </div>
              <div class="flex flex-wrap gap-1 mt-1">
                <span
                  v-for="s in app.scope.split(' ').filter(Boolean)"
                  :key="s"
                  class="inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-primary/10 text-primary"
                >
                  {{ s }}
                </span>
              </div>
            </div>
          </div>
          <UiButton
            size="sm"
            variant="destructive"
            @click="openRevokeDialog(app)"
          >
            撤销
          </UiButton>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-if="apps.length === 0 && !loading" class="bg-card rounded-lg border border-border shadow-sm text-center py-16">
        <svg class="w-16 h-16 text-muted-foreground mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"></path>
        </svg>
        <h3 class="text-lg font-medium text-foreground mb-2">暂无已授权应用</h3>
        <p class="text-muted-foreground">
          您尚未授权任何第三方应用访问您的账户。
        </p>
      </div>
    </div>

    <!-- 撤销确认 -->
    <UiDialog
      v-model:open="confirmingRevoke"
      title="撤销授权"
      @confirm="confirmRevoke"
      @cancel="() => { confirmingRevoke = false; appToRevoke = null }"
    >
      <p class="text-sm text-muted-foreground">
        确认撤销对「<strong>{{ appToRevoke?.client_name }}</strong>」的授权？
        该应用的所有 token 将被立即吊销，且该操作不可恢复。
      </p>
    </UiDialog>
  </div>
</template>
