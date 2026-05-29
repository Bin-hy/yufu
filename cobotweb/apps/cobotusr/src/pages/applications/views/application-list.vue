<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { emitToast } from '@/utils/toast'
import * as appApi from '../api'
import * as oauthApi from '@/pages/oauth/api'
import type { ApplicationVO } from '@/types/models'
import type { ClientVO } from '@/pages/oauth/api/type'
import Pagination from '@/components/ui/pagination/pagination.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'

// --- merged app list ---
interface AppEntry {
  app: ApplicationVO
  client: ClientVO | null
}

const loading = ref(false)
const entries = ref<AppEntry[]>([])
const pageNumber = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)

const router = useRouter()
const route = useRoute()

function updateQuery() {
  router.replace({ query: { ...route.query, page_number: pageNumber.value, page_size: pageSize.value } })
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    emitToast({ type: 'success', message: '已复制' })
  } catch {
    emitToast({ type: 'error', message: '复制失败' })
  }
}

async function loadData() {
  loading.value = true
  try {
    const [appPage, clients] = await Promise.all([
      appApi.getApplicationsByPage(pageNumber.value, pageSize.value),
      oauthApi.listClients().catch(() => [] as ClientVO[]),
    ])
    const apps = appPage.data_list || []
    totalPages.value = appPage.total_pages || 1
    entries.value = apps.map(a => ({
      app: a,
      client: clients.find(c => c.id === a.oidc_client_id) || null,
    }))
  } catch (e) {
    console.error('加载应用列表失败', e)
  } finally {
    loading.value = false
  }
}

// --- create / edit form ---
interface AppForm {
  name: string
  description: string
  redirect_uris: string
  grant_types: string[]
  allowed_scopes: string[]
}

const GRANT_OPTIONS = ['authorization_code', 'refresh_token', 'client_credentials']
const SCOPE_OPTIONS = ['openid', 'profile', 'email', 'phone', 'address', 'offline_access']

function defaultForm(): AppForm {
  return { name: '', description: '', redirect_uris: '', grant_types: ['authorization_code', 'refresh_token'], allowed_scopes: ['openid', 'profile', 'email'] }
}

const dialogOpen = ref(false)
const isEditing = ref(false)
const editingApp = ref<AppEntry | null>(null)
const form = ref(defaultForm())
const submitting = ref(false)

// generated secret (shown once)
const newSecret = ref('')
const showSecret = ref(false)

function openCreate() {
  isEditing.value = false
  editingApp.value = null
  form.value = defaultForm()
  newSecret.value = ''
  showSecret.value = false
  dialogOpen.value = true
}

function openEdit(entry: AppEntry) {
  isEditing.value = true
  editingApp.value = entry
  form.value = {
    name: entry.app.name,
    description: entry.app.description || '',
    redirect_uris: entry.client?.redirect_uris || '',
    grant_types: entry.client?.grant_types ? entry.client.grant_types.split(',').map(s => s.trim()) : ['authorization_code'],
    allowed_scopes: entry.client?.allowed_scopes ? entry.client.allowed_scopes.split(',').map(s => s.trim()) : ['openid', 'profile'],
  }
  newSecret.value = ''
  showSecret.value = false
  dialogOpen.value = true
}

function toggleGrant(g: string) {
  const idx = form.value.grant_types.indexOf(g)
  if (idx > -1) form.value.grant_types.splice(idx, 1)
  else form.value.grant_types.push(g)
}

function toggleScope(s: string) {
  const idx = form.value.allowed_scopes.indexOf(s)
  if (idx > -1) form.value.allowed_scopes.splice(idx, 1)
  else form.value.allowed_scopes.push(s)
}

async function submitForm() {
  if (!form.value.name.trim()) {
    emitToast({ type: 'warning', message: '请填写应用名称' })
    return
  }
  if (!form.value.redirect_uris.trim()) {
    emitToast({ type: 'warning', message: '请填写回调地址' })
    return
  }
  submitting.value = true
  try {
    if (isEditing.value && editingApp.value) {
      const app = editingApp.value.app
      const clientId = app.oidc_client_id
      // update application
      await appApi.patchApplication({ id: app.id, name: form.value.name.trim() || null, description: form.value.description.trim() || null })
      // update oauth client
      if (clientId) {
        await oauthApi.updateClient(clientId, {
          name: form.value.name.trim(),
          redirect_uris: form.value.redirect_uris.split(',').map(s => s.trim()).filter(Boolean),
          allowed_scopes: form.value.allowed_scopes,
          grant_types: form.value.grant_types,
        })
      }
      emitToast({ type: 'success', message: '应用已更新' })
    } else {
      // create: POST /oidc/clients → POST /application
      const clientResp = await oauthApi.createClient({
        name: form.value.name.trim(),
        redirect_uri: form.value.redirect_uris.trim(),
      })
      newSecret.value = clientResp.client_secret
      showSecret.value = true
      // create application row linked to this client
      await appApi.createApplication({
        app_key: clientResp.client_id,
        name: form.value.name.trim(),
        description: form.value.description.trim() || null,
        oidc_client_id: clientResp.id,
      })
      emitToast({ type: 'success', message: '应用注册成功' })
    }
    dialogOpen.value = false
    await loadData()
  } catch (e) {
    console.error('保存应用失败', e)
    emitToast({ type: 'error', message: '保存应用失败' })
  } finally {
    submitting.value = false
  }
}

function onDialogClose() {
  dialogOpen.value = false
  newSecret.value = ''
  showSecret.value = false
}

// --- regenerate secret ---
const regenDialogOpen = ref(false)
const regenTarget = ref<AppEntry | null>(null)
const regenSecret = ref('')
const regenShow = ref(false)
const regenLoading = ref(false)

function openRegen(entry: AppEntry) {
  regenTarget.value = entry
  regenSecret.value = ''
  regenShow.value = false
  regenDialogOpen.value = true
}

async function confirmRegen() {
  if (!regenTarget.value?.app?.oidc_client_id) return
  regenLoading.value = true
  try {
    const resp = await oauthApi.regenerateSecret(regenTarget.value.app.oidc_client_id)
    regenSecret.value = resp.client_secret
    regenShow.value = true
    emitToast({ type: 'success', message: '密钥已重新生成' })
  } catch (e) {
    emitToast({ type: 'error', message: '重新生成密钥失败' })
  } finally {
    regenLoading.value = false
  }
}

// --- delete ---
const deleting = ref(false)
const appToDelete = ref<AppEntry | null>(null)

function openDeleteDialog(entry: AppEntry) {
  appToDelete.value = entry
  deleting.value = true
}

async function confirmDelete() {
  if (!appToDelete.value) return
  try {
    if (appToDelete.value.app.oidc_client_id) {
      await oauthApi.deleteClient(appToDelete.value.app.oidc_client_id)
    }
    await appApi.deleteApplication(appToDelete.value.app.id)
    emitToast({ type: 'success', message: `已删除应用：${appToDelete.value.app.name}` })
    deleting.value = false
    appToDelete.value = null
    await loadData()
  } catch (e) {
    emitToast({ type: 'error', message: '删除应用失败' })
  }
}

onMounted(async () => {
  const pn = Number(route.query.page_number ?? 1)
  const ps = Number(route.query.page_size ?? 10)
  pageNumber.value = Number.isFinite(pn) && pn > 0 ? pn : 1
  pageSize.value = [10, 20, 50].includes(ps) ? ps : 10
  updateQuery()
  await loadData()
})
</script>

<template>
  <div class="space-y-6">
    <!-- header -->
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
      <div>
        <h2 class="text-2xl font-bold text-foreground">应用管理</h2>
        <p class="text-sm text-muted-foreground mt-1">注册并管理接入平台的应用，一个应用 = 一个 OAuth Client = 一个权限命名空间</p>
      </div>
      <UiButton @click="openCreate">
        <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"></path>
        </svg>
        注册应用
      </UiButton>
    </div>

    <!-- list -->
    <div class="space-y-3">
      <div
        v-for="e in entries" :key="e.app.id"
        class="bg-card rounded-lg border border-border shadow-sm hover:border-primary/50 transition-colors"
      >
        <div class="px-6 py-4">
          <div class="flex items-center justify-between">
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 w-full">
              <div class="flex items-center gap-4 min-w-0">
                <div class="w-12 h-12 rounded-lg bg-primary/10 flex items-center justify-center flex-shrink-0">
                  <svg class="w-6 h-6 text-primary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
                  </svg>
                </div>
                <div class="min-w-0">
                  <div class="text-base font-semibold text-foreground truncate">{{ e.app.name }}</div>
                  <div v-if="e.client" class="flex flex-col gap-0.5 mt-1">
                    <div class="flex items-center gap-2 text-xs">
                      <span class="text-muted-foreground shrink-0">Client ID</span>
                      <code class="bg-muted px-1.5 py-0.5 rounded text-xs truncate">{{ e.client.client_id }}</code>
                      <button class="text-primary hover:underline shrink-0 text-xs" @click="copyText(e.client!.client_id)">复制</button>
                    </div>
                    <div class="flex items-center gap-2 text-xs">
                      <span class="text-muted-foreground shrink-0">Secret</span>
                      <code class="bg-muted px-1.5 py-0.5 rounded text-xs">********</code>
                      <button class="text-primary hover:underline shrink-0 text-xs" @click="openRegen(e)">重新生成</button>
                    </div>
                    <div class="flex items-center gap-2 text-xs text-muted-foreground">
                      <span class="shrink-0">回调</span>
                      <span class="truncate">{{ e.client.redirect_uris.split(',')[0] || '-' }}</span>
                    </div>
                  </div>
                  <div v-else class="text-xs text-yellow-500 mt-1">未关联 OAuth Client</div>
                </div>
              </div>
              <div class="flex items-center gap-2 shrink-0">
                <span class="text-xs text-muted-foreground hidden sm:block">{{ e.app.create_time }}</span>
                <UiButton size="sm" variant="outline" @click="openEdit(e)">编辑</UiButton>
                <UiButton size="sm" variant="destructive" @click="openDeleteDialog(e)">删除</UiButton>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-if="!loading && entries.length === 0" class="bg-card rounded-lg border border-border shadow-sm text-center py-16">
        <svg class="w-16 h-16 text-muted-foreground mx-auto mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
        </svg>
        <h3 class="text-lg font-medium text-foreground mb-2">还没有注册应用</h3>
        <p class="text-muted-foreground mb-4">注册一个应用来接入平台的身份认证和权限管理</p>
        <UiButton @click="openCreate">注册应用</UiButton>
      </div>
      <div v-if="loading" class="p-4 text-center text-sm text-muted-foreground">正在加载...</div>
    </div>

    <!-- pagination -->
    <div class="flex justify-center">
      <Pagination
        :page="pageNumber" :page-count="totalPages" :size="pageSize"
        @page-change="(p:number) => { pageNumber = p; updateQuery(); loadData() }"
        @update:size="(s:number) => { pageSize = s; pageNumber = 1; updateQuery(); loadData() }"
      />
    </div>

    <!-- create / edit dialog -->
    <UiDialog v-model:open="dialogOpen" :title="isEditing ? '编辑应用' : '注册新应用'" @confirm="submitForm" @cancel="onDialogClose">
      <div class="space-y-4">
        <div>
          <label class="block text-sm mb-1">应用名称 <span class="text-destructive">*</span></label>
          <UiInput v-model="form.name" placeholder="例如：CoBotrix 管理端" />
        </div>
        <div>
          <label class="block text-sm mb-1">描述</label>
          <UiInput v-model="form.description" placeholder="可选描述" />
        </div>
        <div>
          <label class="block text-sm mb-1">授权回调地址 <span class="text-destructive">*</span></label>
          <UiInput v-model="form.redirect_uris" placeholder="http://localhost:5173/oauth/callback（多个用逗号分隔）" />
        </div>
        <div>
          <label class="block text-sm mb-1">授权方式</label>
          <div class="flex flex-wrap gap-2">
            <button
              v-for="g in GRANT_OPTIONS" :key="g" type="button"
              class="px-3 py-1 text-xs rounded-full border transition-colors"
              :class="form.grant_types.includes(g) ? 'bg-primary text-primary-foreground border-primary' : 'border-border text-muted-foreground hover:border-primary/50'"
              @click="toggleGrant(g)"
            >{{ g }}</button>
          </div>
        </div>
        <div>
          <label class="block text-sm mb-1">权限范围</label>
          <div class="flex flex-wrap gap-2">
            <button
              v-for="s in SCOPE_OPTIONS" :key="s" type="button"
              class="px-3 py-1 text-xs rounded-full border transition-colors"
              :class="form.allowed_scopes.includes(s) ? 'bg-primary text-primary-foreground border-primary' : 'border-border text-muted-foreground hover:border-primary/50'"
              @click="toggleScope(s)"
            >{{ s }}</button>
          </div>
        </div>
        <p v-if="submitting" class="text-xs text-muted-foreground">提交中...</p>
        <!-- new secret display -->
        <div v-if="showSecret && newSecret" class="rounded-md border border-green-500/30 bg-green-500/10 p-3">
          <p class="text-sm font-medium text-green-700 mb-1">应用密钥已生成（仅显示一次）</p>
          <code class="text-xs bg-muted px-2 py-1 rounded break-all">{{ newSecret }}</code>
        </div>
      </div>
    </UiDialog>

    <!-- regenerate secret dialog -->
    <UiDialog v-model:open="regenDialogOpen" title="重新生成密钥" hide-footer @cancel="() => { regenDialogOpen = false }">
      <div class="space-y-4">
        <p class="text-sm text-muted-foreground">
          重新生成密钥后，旧密钥将立即失效。使用旧密钥的应用将无法进行 token 交换。
        </p>
        <div v-if="regenShow && regenSecret" class="rounded-md border border-green-500/30 bg-green-500/10 p-3">
          <div class="flex items-center gap-2 mb-1">
            <p class="text-sm font-medium text-green-700">新密钥（仅显示一次）</p>
            <button class="text-xs text-primary hover:underline shrink-0" @click="copyText(regenSecret)">复制</button>
          </div>
          <code class="text-xs bg-muted px-2 py-1 rounded break-all">{{ regenSecret }}</code>
        </div>
        <div class="flex justify-end gap-2">
          <button
            class="inline-flex h-9 items-center justify-center rounded-xl border border-input bg-background px-4 text-sm font-medium text-foreground hover:bg-accent transition-colors"
            @click="regenDialogOpen = false"
          >
            {{ regenShow ? '关闭' : '取消' }}
          </button>
          <button
            v-if="!regenShow"
            class="inline-flex h-9 items-center justify-center rounded-xl bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
            :disabled="regenLoading"
            @click="confirmRegen"
          >
            {{ regenLoading ? '生成中...' : '确认重新生成' }}
          </button>
        </div>
      </div>
    </UiDialog>

    <!-- delete dialog -->
    <UiDialog v-model:open="deleting" title="确认删除" @confirm="confirmDelete" @cancel="() => { deleting = false; appToDelete = null }">
      <p class="text-sm text-muted-foreground">
        确认删除应用「<strong>{{ appToDelete?.app.name }}</strong>」？所有关联的 token 将立即失效，该操作不可恢复。
      </p>
    </UiDialog>
  </div>
</template>
