<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import UiButton from '@/components/ui/button/button.vue'
import type { UserVO, RecordingVO } from '@/types/models'
import type { UsersPage } from '@/pages/users/api/type'
import { getUsersByPage } from '@/pages/users/api'
import { get_recording_by_page, get_recording_by_user_id_page } from '@/pages/recordings/api'

// 查询状态
const userSearch = ref('')
const selectedUserId = ref<number | null>(null)
const pageNumber = ref(1)
const pageSize = ref(10)
const totalPages = ref(1)
const loading = ref(false)

// 数据
const allUsers = ref<UserVO[]>([])
const recordings = ref<RecordingVO[]>([])

// 本地过滤后的用户列表
const filteredUsers = computed(() => {
  const term = userSearch.value.trim().toLowerCase()
  if (!term) return allUsers.value
  return allUsers.value.filter(u =>
    (u.username && u.username.toLowerCase().includes(term)) ||
    (u.nickname && u.nickname.toLowerCase().includes(term))
  )
})

// 加载全部用户（分页聚合）
async function loadAllUsers() {
  const pageSizeOnce = 100
  let pn = 1
  let next = true
  const acc: UserVO[] = []
  while (next) {
    const page: UsersPage = await getUsersByPage(pn, pageSizeOnce)
    acc.push(...page.data_list)
    pn++
    next = pn <= page.total_pages
  }
  allUsers.value = acc
}

// 加载记录（全部或指定用户）
async function loadRecordings() {
  loading.value = true
  try {
    if (selectedUserId.value != null) {
      const page = await get_recording_by_user_id_page(selectedUserId.value, pageNumber.value, pageSize.value)
      recordings.value = page.data_list
      totalPages.value = page.total_pages
    } else {
      const page = await get_recording_by_page(pageNumber.value, pageSize.value)
      recordings.value = page.data_list
      totalPages.value = page.total_pages
    }
  } finally {
    loading.value = false
  }
}

function clearSelection() {
  selectedUserId.value = null
  pageNumber.value = 1
  loadRecordings()
}

function prevPage() {
  if (pageNumber.value > 1) {
    pageNumber.value -= 1
  }
}

function nextPage() {
  if (pageNumber.value < totalPages.value) {
    pageNumber.value += 1
  }
}

watch([selectedUserId, pageNumber, pageSize], () => {
  loadRecordings()
})

onMounted(async () => {
  await loadAllUsers()
  await loadRecordings()
})
</script>

<template>
  <div class="rounded-lg shadow overflow-hidden bg-card text-card-foreground border border-border">
    <!-- 页面标题与操作 -->
    <div class="px-6 py-4 border-b border-border">
      <div class="flex justify-between items-center">
        <h2 class="text-lg font-medium">操作记录</h2>
        <div class="flex items-center space-x-2">
          <UiButton variant="outline" @click="clearSelection">查看所有</UiButton>
        </div>
      </div>
    </div>

    <!-- 过滤与选择 -->
    <div class="px-6 py-4 border-b border-border">
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div>
          <label class="block text-sm font-medium mb-1">搜索用户</label>
          <UiInput v-model="userSearch" placeholder="输入用户名或昵称（本地过滤）" />
        </div>
        <div class="md:col-span-2">
          <label class="block text-sm font-medium mb-1">选择用户</label>
          <UiSelect v-model.number="selectedUserId" class="w-full">
            <option :value="0">全部用户</option>
            <option v-for="u in filteredUsers" :key="u.id" :value="u.id">
              {{ u.nickname || u.username }}（ID: {{ u.id }}）
            </option>
          </UiSelect>
        </div>
      </div>
    </div>

    <!-- 列表 -->
    <div class="px-6 py-4">
      <div class="overflow-x-auto">
        <table class="min-w-full text-sm">
          <thead>
            <tr class="text-left">
              <th class="px-3 py-2">ID</th>
              <th class="px-3 py-2">时间</th>
              <th class="px-3 py-2">用户</th>
              <th class="px-3 py-2">操作名称</th>
              <th class="px-3 py-2">权限 URI</th>
            </tr>
          </thead>
          <tbody>
            <tr v-if="loading">
              <td colspan="5" class="px-3 py-4">加载中...</td>
            </tr>
            <tr v-else-if="recordings.length === 0">
              <td colspan="5" class="px-3 py-4">暂无数据</td>
            </tr>
            <tr v-else v-for="r in recordings" :key="r.id" class="border-t border-border">
              <td class="px-3 py-2">{{ r.id }}</td>
              <td class="px-3 py-2">{{ r.create_time }}</td>
              <td class="px-3 py-2">{{ r.user_vo?.nickname || r.user_vo?.username }}（ID: {{ r.user_id }}）</td>
              <td class="px-3 py-2">{{ r.name }}</td>
              <td class="px-3 py-2">{{ r.authority_uri }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 分页 -->
      <div class="flex justify-between items-center mt-4">
        <div class="flex items-center space-x-2">
          <span class="text-sm">每页</span>
          <UiSelect v-model.number="pageSize" class="w-24">
            <option :value="10">10</option>
            <option :value="20">20</option>
            <option :value="50">50</option>
          </UiSelect>
        </div>
        <div class="flex items-center space-x-2">
          <UiButton variant="outline" :disabled="pageNumber <= 1" @click="prevPage">上一页</UiButton>
          <span class="text-sm">第 {{ pageNumber }} / {{ totalPages }} 页</span>
          <UiButton variant="outline" :disabled="pageNumber >= totalPages" @click="nextPage">下一页</UiButton>
        </div>
      </div>
    </div>
  </div>
</template>