<script setup lang="ts">
import { computed, ref } from 'vue'
import { RefreshCw } from '@lucide/vue'
import { get_recording_by_page, get_recording_by_user_id_page } from '../api'
import { getUsersByPage } from '@/pages/users/api'
import { usePagedList } from '@/composables/use-paged-list'
import { usePagedOptions } from '@/composables/use-paged-options'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
import Pagination from '@/components/ui/pagination/pagination.vue'
const list = usePagedList((q, signal) =>
        q.user_id
            ? get_recording_by_user_id_page(q.user_id, q.page_number, q.page_size, {
                  signal,
                  errorPresentation: 'local',
              })
            : get_recording_by_page(q.page_number, q.page_size, {
                  signal,
                  errorPresentation: 'local',
              }),
    ),
    { rows, page, size, totalPages, status, error, refreshing, query } = list
const users = usePagedOptions((p, s, signal) =>
        getUsersByPage(p, s, { signal, errorPresentation: 'local' }),
    ),
    search = ref('')
const options = computed(() =>
    users.items.value.filter((user) =>
        (user.username + ' ' + (user.nickname ?? ''))
            .toLowerCase()
            .includes(search.value.toLowerCase()),
    ),
)
</script>
<template>
    <section class="workspace-page">
        <PageHeader
            title="操作记录"
            description="按账户查看平台中的接口访问与操作历史。"
            eyebrow="ACTIVITY"
            ><template #actions
                ><UiButton variant="outline" :disabled="refreshing" @click="list.refresh"
                    ><RefreshCw :size="15" aria-hidden="true" />刷新</UiButton
                ></template
            ></PageHeader
        >
        <div class="panel">
            <div class="query-toolbar">
                <UiInput
                    v-model="search"
                    class="search-field"
                    placeholder="搜索已加载的用户…"
                    aria-label="搜索用户选项"
                /><UiSelect
                    class="!w-60 max-w-full"
                    :model-value="query.user_id ?? null"
                    aria-label="按用户筛选"
                    @update:model-value="
                        (value) => list.setFilters({ user_id: Number(value) || undefined })
                    "
                    ><option :value="null">全部用户</option>
                    <option
                        v-if="query.user_id && !options.some((user) => user.id === query.user_id)"
                        :value="query.user_id"
                    >
                        用户 #{{ query.user_id }}
                    </option>
                    <option v-for="user in options" :key="user.id" :value="user.id">
                        {{ user.nickname || user.username }} · #{{ user.id }}
                    </option></UiSelect
                ><UiButton
                    v-if="users.hasMore.value"
                    variant="outline"
                    size="sm"
                    :disabled="users.loading.value"
                    @click="users.loadNext"
                    >{{ users.loading.value ? '加载中…' : '加载更多用户' }}</UiButton
                ><UiButton
                    v-if="query.user_id"
                    variant="ghost"
                    @click="list.setFilters({ user_id: undefined })"
                    >清除筛选</UiButton
                >
                <p v-if="users.error.value" class="inline-error" role="alert">
                    用户选项加载失败。<button type="button" class="underline" @click="users.retry">
                        重试
                    </button>
                </p>
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!rows.length"
                @retry="list.refresh"
                ><div class="table-scroll">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th scope="col">操作</th>
                                <th scope="col">用户</th>
                                <th scope="col">方法</th>
                                <th scope="col">接口标识</th>
                                <th scope="col">发生时间</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr v-for="row in rows" :key="row.id">
                                <td>
                                    <p class="font-medium">{{ row.name }}</p>
                                    <p class="text-xs text-muted-foreground mt-1">#{{ row.id }}</p>
                                </td>
                                <td>
                                    {{ row.user_vo?.nickname || row.user_vo?.username || '用户' }} ·
                                    #{{ row.user_id }}
                                </td>
                                <td>
                                    <span class="status-badge">{{ row.method }}</span>
                                </td>
                                <td>
                                    <code class="text-xs">{{ row.authority_uri }}</code>
                                </td>
                                <td class="text-xs text-muted-foreground">{{ row.create_time }}</td>
                            </tr>
                        </tbody>
                    </table>
                </div></DataState
            ><Pagination
                :page="page"
                :size="size"
                :page-count="totalPages"
                @page-change="list.setPage"
                @update:size="list.setSize"
            />
        </div>
    </section>
</template>
