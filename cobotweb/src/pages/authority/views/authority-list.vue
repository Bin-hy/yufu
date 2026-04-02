<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue';
import * as authorityApi from '../api';
import Pagination from '@/components/ui/pagination/pagination.vue'
import type { PageResult } from '@/types/pagination'
import { useRoute, useRouter } from 'vue-router'
import type { AuthorityVO } from '@/types/models';
import UiInput from '@/components/ui/input/input.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'

const page = ref<PageResult<AuthorityVO>>({ page_number: 1, page_size: 10, total_pages: 1, data_list: [] })
// 加载状态
const loading = ref(false);

const router = useRouter()
const route = useRoute()
function updateQuery() {
  router.replace({ query: { ...route.query, page_number: page.value.page_number, page_size: page.value.page_size } })
}

// 获取权限列表
const fetchAuthorities = async () => {
  loading.value = true;
  try {
    page.value = await authorityApi.getAuthoritiesByPage(page.value.page_number, page.value.page_size);
  } catch (error) {
    console.error('获取权限列表失败');
  } finally {
    loading.value = false;
  }
};

// 初始化
onMounted(async () => {
  const pn = Number(route.query.page_number ?? 1)
  const ps = Number(route.query.page_size ?? 10)
  page.value.page_number = Number.isFinite(pn) && pn > 0 ? pn : 1
  page.value.page_size = [10, 20, 50].includes(ps) ? ps : 10
  updateQuery()
  await fetchAuthorities();
});

// 添加权限表单
const addForm = reactive({
  name: '',
  authority_uri: '',
  method: 'GET',
})
const addErrors = reactive({ name: '', authority_uri: '', method: '' })
const adding = ref(false)
const dialogOpen = ref(false)

function resetAddForm() {
  addForm.name = ''
  addForm.authority_uri = ''
  addForm.method = 'GET'
  addErrors.name = ''
  addErrors.authority_uri = ''
  addErrors.method = ''
}

function validateAddForm(): boolean {
  addErrors.name = ''
  addErrors.authority_uri = ''
  addErrors.method = ''
  let ok = true
  if (!addForm.name.trim()) { addErrors.name = '请输入权限名称'; ok = false }
  if (!addForm.authority_uri.trim()) { addErrors.authority_uri = '请输入权限标识'; ok = false }
  if (!addForm.method.trim()) { addErrors.method = '请选择请求方法'; ok = false }
  return ok
}

// 由弹窗确认触发提交

function onCancelAdd() {
  resetAddForm()
}

async function onConfirmAdd() {
  // UiDialog 在 confirm 后会自动关闭，这里如果校验失败则立刻重新打开以避免误关闭
  if (!validateAddForm()) {
    dialogOpen.value = true
    return
  }
  try {
    adding.value = true
    await authorityApi.createAuthority({
      name: addForm.name.trim(),
      authority_uri: addForm.authority_uri.trim(),
      method: addForm.method.trim(),
    })
    resetAddForm()
    await fetchAuthorities()
  } catch (e) {
    console.error('添加权限失败', e)
    // 失败时重新打开便于用户修正
    dialogOpen.value = true
  } finally {
    adding.value = false
  }
}

</script>

<template>
  <div class="rounded-lg shadow-sm overflow-hidden bg-card text-card-foreground border border-border">
    <!-- 工具栏 -->
    <div class="p-4 border-b border-border flex justify-between items-center">
      <h2 class="font-semibold">系统权限</h2>
      <UiButton @click="dialogOpen = true">新增权限</UiButton>
    </div>

    <!-- 权限表格 -->
    <div class="overflow-x-auto">
      <table class="min-w-full divide-y divide-border text-sm">
        <thead>
          <tr>
            <th scope="col" class="px-4 py-2 text-left">权限ID</th>
            <th scope="col" class="px-4 py-2 text-left">权限名称</th>
            <th scope="col" class="px-4 py-2 text-left">权限标识</th>
            <th scope="col" class="px-4 py-2 text-left">创建时间</th>
            <th scope="col" class="px-4 py-2 text-left">更新时间</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="page.data_list.length === 0" class="text-center">
            <td colspan="6" class="px-6 py-12 text-muted-foreground">暂无权限数据</td>
          </tr>
          <tr v-for="authority in page.data_list" :key="authority.id">
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.id }}</td>
            <td class="px-4 py-2 text-sm font-medium">{{ authority.name }}</td>
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.authority_uri }}</td>
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.create_time }}</td>
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.update_time }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <Pagination
      :page="page.page_number"
      :page-count="page.total_pages"
      :size="page.page_size"
      @page-change="(p:number) => { page.page_number = p; updateQuery(); fetchAuthorities(); }"
      @update:size="(s:number) => { page.page_size = s; page.page_number = 1; updateQuery(); fetchAuthorities(); }"
    />

    <!-- 加载状态 -->
    <div v-if="loading && page.data_list.length === 0" class="py-12 text-center">
      <div class="inline-block animate-spin rounded-full h-8 w-8 border-4 border-border border-t-primary"></div>
      <p class="mt-2 text-muted-foreground">加载中...</p>
    </div>
  </div>

  <!-- 新增权限弹窗 -->
  <UiDialog v-model:open="dialogOpen" title="新增权限" @confirm="onConfirmAdd" @cancel="onCancelAdd">
    <div class="space-y-3">
      <div>
        <label class="block text-xs font-medium mb-1">权限名称</label>
        <UiInput v-model="addForm.name" placeholder="例如：查看用户" class="w-full" />
        <p v-if="addErrors.name" class="text-xs text-destructive mt-1">{{ addErrors.name }}</p>
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">权限标识（URI）</label>
        <UiInput v-model="addForm.authority_uri" placeholder="例如：/user/view" class="w-full" />
        <p v-if="addErrors.authority_uri" class="text-xs text-destructive mt-1">{{ addErrors.authority_uri }}</p>
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">方法</label>
        <select v-model="addForm.method" class="border border-input rounded-md h-9 px-3 text-sm w-full">
          <option value="GET">GET</option>
          <option value="POST">POST</option>
          <option value="PUT">PUT</option>
          <option value="PATCH">PATCH</option>
          <option value="DELETE">DELETE</option>
        </select>
        <p v-if="addErrors.method" class="text-xs text-destructive mt-1">{{ addErrors.method }}</p>
      </div>
      <p v-if="adding" class="text-xs text-muted-foreground">提交中，请稍候...</p>
    </div>
  </UiDialog>
</template>
