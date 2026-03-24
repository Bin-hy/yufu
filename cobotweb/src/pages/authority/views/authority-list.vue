<script setup lang="ts">
import { ref, onMounted } from 'vue';
import * as authorityApi from '../api';
import Pagination from '@/components/ui/pagination/pagination.vue'
import type { PageResult } from '@/types/pagination'
import { useRoute, useRouter } from 'vue-router'
import type { AuthorityVO } from '@/types/models';

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

</script>

<template>
  <div class="rounded-lg shadow-sm overflow-hidden bg-card text-card-foreground border border-border">
    <!-- 工具栏 -->
    <div class="p-4 border-b border-border flex justify-between items-center">
      <h2 class="font-semibold">系统权限</h2>

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
</template>
