<script setup lang="ts">
defineOptions({ name: 'AccountApps' })
import { computed, ref, watch, onBeforeUnmount } from 'vue'
import { AppWindow, FlaskConical, RefreshCw, Search, ShieldCheck } from '@lucide/vue'
import { getApplications } from '../api'
import { useCursorPage } from '@/composables/use-cursor-page'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiSelect from '@/components/ui/select/select.vue'
const query = ref('')
const debouncedQuery = ref('')
const environment = ref<'production' | 'development'>('production')
let timer: number | undefined
watch(query, value => {
    window.clearTimeout(timer)
    timer = window.setTimeout(() => { debouncedQuery.value = value.trim() }, 240)
})
onBeforeUnmount(() => window.clearTimeout(timer))
const state = useCursorPage(() => `personal-apps:${environment.value}:${debouncedQuery.value}`, (after, signal) => getApplications(after, signal, environment.value, debouncedQuery.value || undefined)),
    { data, status, error, refreshing } = state
watch([environment, debouncedQuery], () => state.first())
const filtered = computed(() => data.value?.items ?? [])
const productionCount = computed(
    () => data.value?.items.filter((app) => app.environment === 'production').length ?? 0,
)
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="可访问应用"
            eyebrow="ASSIGNMENTS"
            description="企业已明确分配给你的应用环境；应用维护关系仅提供开发环境准入，不等于生产访问授权。"
        >
            <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="state.refresh()">
                <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />刷新
            </UiButton>
        </PageHeader>

        <div class="metric-grid mb-5">
            <article class="metric-card">
                <span class="metric-label"><AppWindow :size="13" aria-hidden="true" />已分配环境</span>
                <strong class="metric-value">{{ data?.items.length ?? 0 }}</strong>
                <span class="metric-meta">当前页面加载的环境数量</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><ShieldCheck :size="13" aria-hidden="true" />生产环境</span>
                <strong class="metric-value">{{ productionCount }}</strong>
                <span class="metric-meta">需要管理员明确分配</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><FlaskConical :size="13" aria-hidden="true" />开发环境</span>
                <strong class="metric-value">{{ (data?.items.length ?? 0) - productionCount }}</strong>
                <span class="metric-meta">开发者维护关系的准入范围</span>
            </article>
        </div>

        <section class="panel">
            <div class="table-toolbar">
                <UiSelect v-model="environment" aria-label="应用环境">
                    <option value="production">生产环境</option>
                    <option value="development">开发环境</option>
                </UiSelect>
                <div class="search-box">
                    <Search :size="15" aria-hidden="true" />
                    <UiInput v-model="query" clearable placeholder="搜索应用或环境…" aria-label="搜索应用" />
                </div>
                <span class="toolbar-spacer text-[11px] text-muted-foreground">
                    共 {{ filtered.length }} 个结果
                </span>
            </div>
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!data?.items.length"
                empty-title="还没有可访问的应用"
                empty-description="联系企业管理员，为你的账户分配应用环境访问权。"
                :skeleton-rows="2"
                @retry="state.refresh()"
            >
                <div v-if="filtered.length" class="grid gap-4 p-4 md:grid-cols-2">
                    <article
                        v-for="app in filtered"
                        :key="app.environment_id"
                        class="app-card flex-col gap-4"
                    >
                        <div class="flex items-start justify-between gap-3">
                            <span class="list-icon">
                                <AppWindow :size="17" aria-hidden="true" />
                            </span>
                            <span
                                class="status-badge"
                                :class="app.environment === 'production' ? 'status-success' : 'status-info'"
                            >
                                {{ app.environment === 'production' ? '生产环境' : '开发环境' }}
                            </span>
                        </div>
                        <div>
                            <h2 class="text-sm font-semibold">{{ app.name }}</h2>
                            <p class="mt-1 text-[11.5px] leading-5 text-muted-foreground">
                                {{ app.launch_description || '请从应用自己的登录入口发起访问。' }}
                            </p>
                        </div>
                        <ul v-if="app.sources?.length" class="text-xs text-muted-foreground" aria-label="访问来源">
                            <li v-for="source in app.sources" :key="source.kind + ':' + source.source_id">
                                {{ source.kind === 'group' ? '用户组：' : '' }}{{ source.label }}
                            </li>
                        </ul>
                        <a v-if="app.launchable" :href="`/api/v1/account/apps/${encodeURIComponent(app.environment_id)}/launch`" class="text-sm font-medium text-primary underline" referrerpolicy="no-referrer">打开应用</a>
                        <p v-else class="text-xs text-muted-foreground">尚未配置启动入口，请联系应用维护者。</p>
                        <div class="mt-auto flex items-center gap-2 border-t border-border pt-3 text-[10.5px] text-muted-foreground">
                            <ShieldCheck :size="12" aria-hidden="true" />
                            环境 ID <span class="mono">{{ app.environment_id }}</span>
                        </div>
                    </article>
                </div>
                <p v-else class="empty-filter">没有匹配“{{ query }}”的应用，试试其他关键词。</p>
                <div v-if="data?.next" class="flex justify-center border-t border-border p-4">
                    <UiButton variant="outline" :loading="refreshing" :disabled="refreshing" @click="state.next()">
                        加载更多
                    </UiButton>
                </div>
            </DataState>
        </section>
    </div>
</template>
