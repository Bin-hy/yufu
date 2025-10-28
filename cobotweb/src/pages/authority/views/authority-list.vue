<script setup lang="ts">
import { ref, onMounted } from 'vue';
// 暂时注释掉，避免构建错误
// import { ElMessage, ElMessageBox } from 'element-plus';
import * as authorityApi from '../api';
import type { Authority, AuthorityInsertReq, AuthorityUpdateReq } from '../api/type';
import Pagination from '@/components/pagination.vue'
import type { PageResult } from '@/types/pagination'
import { useRoute, useRouter } from 'vue-router'
import UiButton from '@/components/ui/button/button.vue'
import UiDialog from '@/components/ui/dialog/dialog.vue'
import UiInput from '@/components/ui/input/input.vue'

// 权限列表数据
const authorities = ref<Authority[]>([]);
const page = ref<PageResult<Authority>>({ page_number: 1, page_size: 10, total_pages: 1, data_list: [] })
// 加载状态
const loading = ref(false);
// 当前操作的权限
const currentAuthority = ref<Authority | null>(null);
// 显示添加/编辑对话框
const showDialog = ref(false);
// 对话框标题
const dialogTitle = ref('');
// 权限表单数据
const authorityForm = ref<AuthorityInsertReq>({
  name: '',
  authority: ''
});

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
    authorities.value = page.value.data_list
  } catch (error) {
    console.error('获取权限列表失败');
  } finally {
    loading.value = false;
  }
};

// 删除权限
const deleteAuthority = async (id: string, name: string) => {
  try {
    if (!confirm(`确定要删除权限「${name}」吗？删除后无法恢复。`)) {
      return;
    }

    loading.value = true;
    const shouldGoPrev = page.value.data_list.length === 1 && page.value.page_number > 1
    const success = await authorityApi.deleteAuthority(id);
    if (success) {
      if (shouldGoPrev) {
        page.value.page_number = page.value.page_number - 1
        updateQuery()
      }
      await fetchAuthorities();
    } else {
      console.warn('权限删除未成功');
    }
  } catch (error) {
    if (error !== 'cancel') {
      console.error('删除权限操作失败:', error);
    }
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

// 打开添加权限对话框
const openAddDialog = () => {
  currentAuthority.value = null;
  authorityForm.value = { name: '', authority: '' };
  dialogTitle.value = '添加权限';
  showDialog.value = true;
};

// 打开编辑权限对话框
const openEditDialog = (authority: Authority) => {
  currentAuthority.value = authority;
  authorityForm.value = { name: authority.name, authority: authority.authority };
  dialogTitle.value = '编辑权限';
  showDialog.value = true;
};

// 保存权限
const saveAuthority = async () => {
  if (!authorityForm.value.name.trim()) {
    console.warn('请输入权限名称');
    return;
  }
  if (!authorityForm.value.authority.trim()) {
    console.warn('请输入权限标识');
    return;
  }

  loading.value = true;
  try {
    if (currentAuthority.value) {
      await authorityApi.updateAuthority(currentAuthority.value.id, {
        id: currentAuthority.value.id,
        ...authorityForm.value
      });
      console.log('更新成功');
    } else {
      await authorityApi.createAuthority(authorityForm.value);
      console.log('添加成功');
    }
    showDialog.value = false;
    await fetchAuthorities();
  } catch (error) {
    console.error('操作失败');
  } finally {
    loading.value = false;
  }
};
</script>

<template>
  <div class="rounded-lg shadow-sm overflow-hidden bg-card text-card-foreground border border-border">
    <!-- 工具栏 -->
    <div class="p-4 border-b border-border flex justify-between items-center">
      <h2 class="font-semibold">系统权限</h2>
      <UiButton @click="openAddDialog">
        <svg class="inline-block w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"></path>
        </svg>
        添加权限
      </UiButton>
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
            <th scope="col" class="px-4 py-2 text-left">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="page.data_list.length === 0" class="text-center">
            <td colspan="6" class="px-6 py-12 text-muted-foreground">暂无权限数据</td>
          </tr>
          <tr v-for="authority in page.data_list" :key="authority.id">
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.id }}</td>
            <td class="px-4 py-2 text-sm font-medium">{{ authority.name }}</td>
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.authority }}</td>
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.createTime }}</td>
            <td class="px-4 py-2 text-sm text-muted-foreground">{{ authority.updateTime }}</td>
            <td class="px-4 py-2 text-sm">
              <UiButton size="sm" variant="ghost" class="mr-2" @click="openEditDialog(authority)">编辑</UiButton>
              <UiButton size="sm" variant="destructive" @click="deleteAuthority(authority.id, authority.name)">删除</UiButton>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <Pagination
      :page-number="page.page_number"
      :total-pages="page.total_pages"
      :page-size="page.page_size"
      @change="(p:number) => { page.page_number = p; updateQuery(); fetchAuthorities(); }"
      @size-change="(s:number) => { page.page_size = s; page.page_number = 1; updateQuery(); fetchAuthorities(); }"
    />

    <!-- 加载状态 -->
    <div v-if="loading && page.data_list.length === 0" class="py-12 text-center">
      <div class="inline-block animate-spin rounded-full h-8 w-8 border-4 border-border border-t-primary"></div>
      <p class="mt-2 text-muted-foreground">加载中...</p>
    </div>
  </div>

  <!-- 添加/编辑权限对话框 -->
  <UiDialog v-model:open="showDialog" :title="dialogTitle" @cancel="() => (showDialog=false)" @confirm="saveAuthority">
    <div class="space-y-4">
      <div>
        <label class="block text-sm font-medium mb-1">权限名称</label>
        <UiInput v-model="authorityForm.name" type="text" placeholder="请输入权限名称" />
      </div>
      <div>
        <label class="block text-sm font-medium mb-1">权限标识 (URI)</label>
        <UiInput v-model="authorityForm.authority" type="text" placeholder="请输入权限标识，如 /users" />
      </div>
      <div v-if="loading" class="text-xs text-muted-foreground">保存中…</div>
    </div>
  </UiDialog>
</template>
