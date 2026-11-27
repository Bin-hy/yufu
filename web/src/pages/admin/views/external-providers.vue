<script setup lang="ts">
defineOptions({ name: 'AdminExternalProviders' })
import { computed, onMounted, onScopeDispose, ref } from 'vue'
import { RefreshCw, ShieldCheck } from '@lucide/vue'
import {
    getExternalProviderStats,
    getExternalProvidersAdmin,
    rotateExternalProviderSecret,
    updateExternalProvider,
    type ExternalProviderAdminView,
    type ExternalProviderStats,
    type ProviderStat,
} from '../api/external-providers'
import { emitToast } from '@/utils/toast'
import ExternalProviderIcon from '@/components/brand/external-provider-icon.vue'
import PageHeader from '@/components/workspace/page-header.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiCheckbox from '@/components/ui/checkbox/checkbox.vue'
import UiInput from '@/components/ui/input/input.vue'

interface Draft {
    client_id: string
    enabled: boolean
    secret: string
    revision: number
}

const labels: Record<string, string> = {
    github: 'GitHub',
    qq: 'QQ',
    wechat_qr: '微信扫码',
}
const supported = new Set(['github', 'qq', 'wechat_qr'])
const reasonLabels: Record<string, string> = {
    external_identity_not_linked: '未绑定账号',
    unauthenticated: '授权失败/取消',
    dependency_unavailable: '上游不可用',
    invalid_input: '请求无效',
    capacity_exceeded: '限流/繁忙',
}

const providers = ref<ExternalProviderAdminView[]>([])
const drafts = ref<Record<string, Draft>>({})
const stats = ref<ExternalProviderStats | null>(null)
const statsWindow = ref<'24h' | '7d'>('24h')
const loading = ref(true)
const busy = ref('')
const error = ref('')
const controller = new AbortController()

const statMap = computed(() => {
    const map = new Map<string, ProviderStat>()
    for (const item of stats.value?.providers ?? []) map.set(item.provider_key, item)
    return map
})

function apply(view: ExternalProviderAdminView) {
    const index = providers.value.findIndex(provider => provider.provider_key === view.provider_key)
    if (index >= 0) providers.value[index] = view
    drafts.value[view.provider_key] = {
        client_id: view.client_id,
        enabled: view.enabled,
        secret: '',
        revision: view.revision,
    }
}

async function load() {
    loading.value = true
    error.value = ''
    try {
        const rows = await getExternalProvidersAdmin(controller.signal)
        providers.value = rows.filter(row => supported.has(row.provider_key))
        drafts.value = Object.fromEntries(
            providers.value.map(provider => [
                provider.provider_key,
                { client_id: provider.client_id, enabled: provider.enabled, secret: '', revision: provider.revision },
            ]),
        )
        await loadStats()
    } catch (e) {
        if (!controller.signal.aborted) error.value = e instanceof Error ? e.message : '无法读取第三方登录配置'
    } finally {
        loading.value = false
    }
}

async function loadStats() {
    try {
        stats.value = await getExternalProviderStats(statsWindow.value, controller.signal)
    } catch {
        if (!controller.signal.aborted) stats.value = null
    }
}

onMounted(load)
onScopeDispose(() => controller.abort())

async function save(provider: ExternalProviderAdminView) {
    const draft = drafts.value[provider.provider_key]
    if (!draft || busy.value) return
    if (draft.enabled && !draft.client_id.trim()) {
        emitToast({ type: 'error', message: `启用 ${labels[provider.provider_key]} 前必须填写 Client ID/AppID` })
        return
    }
    busy.value = provider.provider_key
    try {
        const view = await updateExternalProvider(provider.provider_key, {
            expected_revision: draft.revision,
            enabled: draft.enabled,
            client_id: draft.client_id.trim(),
            ...(draft.secret ? { secret: draft.secret } : {}),
        })
        apply(view)
        emitToast({ type: 'success', message: `${labels[provider.provider_key]} 配置已保存` })
        await loadStats()
    } catch (e) {
        emitToast({ type: 'error', message: e instanceof Error ? e.message : '保存失败' })
    } finally {
        busy.value = ''
    }
}

async function rotate(provider: ExternalProviderAdminView) {
    const draft = drafts.value[provider.provider_key]
    if (!draft || busy.value) return
    if (!draft.secret) {
        emitToast({ type: 'error', message: '请先输入新的 Secret' })
        return
    }
    busy.value = provider.provider_key
    try {
        const view = await rotateExternalProviderSecret(provider.provider_key, draft.revision, draft.secret)
        apply(view)
        emitToast({ type: 'success', message: `${labels[provider.provider_key]} Secret 已轮换` })
    } catch (e) {
        emitToast({ type: 'error', message: e instanceof Error ? e.message : '轮换失败' })
    } finally {
        busy.value = ''
    }
}

function callbackUrl(key: string) {
    return `${window.location.origin}/identity/external/${key}/callback`
}

function reasonLabel(reason: string) {
    return reasonLabels[reason] ?? reason
}

function date(value: number) {
    return value ? new Date(value * 1000).toLocaleString('zh-CN') : '从未更新'
}
</script>

<template>
    <div class="workspace-page">
        <PageHeader
            title="第三方登录"
            eyebrow="EXTERNAL IDENTITY"
            description="配置 GitHub、QQ、微信开放平台扫码登录。Secret 只写不回显；修改后立即生效。"
        >
            <UiButton variant="outline" :loading="loading" @click="load">
                <RefreshCw :size="14" aria-hidden="true" />刷新
            </UiButton>
        </PageHeader>

        <p v-if="error" role="alert" class="form-alert mb-4">{{ error }}</p>

        <section class="panel mb-5">
            <header class="panel-heading">
                <div>
                    <h2>登录统计</h2>
                    <p>数据来自身份审计，仅统计第三方登录结果。</p>
                </div>
                <div class="flex gap-2">
                    <UiButton
                        v-for="window in (['24h', '7d'] as const)"
                        :key="window"
                        size="sm"
                        :variant="statsWindow === window ? 'default' : 'outline'"
                        @click="statsWindow = window; loadStats()"
                    >
                        {{ window === '24h' ? '24 小时' : '7 天' }}
                    </UiButton>
                </div>
            </header>
            <div class="surface-body grid gap-3 md:grid-cols-3">
                <div v-for="provider in providers" :key="provider.provider_key" class="rounded-xl border border-border p-4">
                    <p class="flex items-center gap-2 text-xs font-semibold">
                        <ExternalProviderIcon :provider="provider.provider_key" :size="15" />
                        {{ labels[provider.provider_key] }}
                    </p>
                    <dl class="mt-3 grid grid-cols-3 gap-2 text-center">
                        <div>
                            <dt class="text-[10px] text-muted-foreground">成功</dt>
                            <dd class="text-lg font-semibold text-success">{{ statMap.get(provider.provider_key)?.success ?? 0 }}</dd>
                        </div>
                        <div>
                            <dt class="text-[10px] text-muted-foreground">拒绝</dt>
                            <dd class="text-lg font-semibold text-warning">{{ statMap.get(provider.provider_key)?.denied ?? 0 }}</dd>
                        </div>
                        <div>
                            <dt class="text-[10px] text-muted-foreground">失败</dt>
                            <dd class="text-lg font-semibold text-destructive">{{ statMap.get(provider.provider_key)?.failure ?? 0 }}</dd>
                        </div>
                    </dl>
                    <p
                        v-for="[reason, count] in (statMap.get(provider.provider_key)?.reasons ?? [])"
                        :key="reason"
                        class="mt-1 flex items-center justify-between text-[10.5px] text-muted-foreground"
                    >
                        <span>{{ reasonLabel(reason) }}</span>
                        <span>{{ count }}</span>
                    </p>
                </div>
            </div>
        </section>

        <div v-if="loading" class="panel surface-body text-sm text-muted-foreground">正在读取 Provider 配置…</div>
        <div v-else class="section-stack">
            <section v-for="provider in providers" :key="provider.provider_key" class="panel">
                <header class="panel-heading">
                    <div>
                        <h2 class="flex items-center gap-2">
                            <span class="flex h-8 w-8 items-center justify-center rounded-lg border border-border bg-background">
                                <ExternalProviderIcon :provider="provider.provider_key" :size="17" />
                            </span>
                            {{ labels[provider.provider_key] }}
                            <span :class="provider.enabled ? 'status-badge status-success' : 'status-badge'">
                                {{ provider.enabled ? '已启用' : '已禁用' }}
                            </span>
                            <span :class="provider.secret_configured ? 'status-badge status-brand' : 'status-badge status-warning'">
                                {{ provider.secret_configured ? 'Secret 已配置' : 'Secret 未配置' }}
                            </span>
                        </h2>
                        <p>回调地址：<span class="mono">{{ callbackUrl(provider.provider_key) }}</span></p>
                    </div>
                </header>
                <div v-if="drafts[provider.provider_key]" class="surface-body space-y-4">
                    <div class="grid gap-4 md:grid-cols-2">
                        <label class="space-y-1.5">
                            <span class="text-xs font-medium">
                                {{ provider.provider_key === 'wechat_qr' ? 'AppID' : 'Client ID' }}
                            </span>
                            <UiInput
                                v-model="drafts[provider.provider_key].client_id"
                                :disabled="busy === provider.provider_key"
                                placeholder="请输入平台分配的应用 ID"
                            />
                        </label>
                        <label class="space-y-1.5">
                            <span class="text-xs font-medium">
                                {{ provider.provider_key === 'wechat_qr' ? 'AppSecret' : 'Client Secret' }}
                            </span>
                            <UiInput
                                v-model="drafts[provider.provider_key].secret"
                                type="password"
                                :disabled="busy === provider.provider_key"
                                :placeholder="provider.secret_configured ? '已配置；留空保持不变' : '请输入 Secret'"
                                autocomplete="new-password"
                            />
                        </label>
                    </div>
                    <div class="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-3">
                        <label class="flex items-center gap-2 text-xs">
                            <UiCheckbox v-model="drafts[provider.provider_key].enabled" :disabled="busy === provider.provider_key" />
                            启用该登录方式
                        </label>
                        <div class="flex items-center gap-3">
                            <span class="text-[10.5px] text-muted-foreground">
                                revision {{ provider.revision }} · 更新于 {{ date(provider.updated_at) }}
                            </span>
                            <UiButton
                                variant="outline"
                                :disabled="busy === provider.provider_key"
                                :loading="busy === provider.provider_key"
                                @click="rotate(provider)"
                            >
                                轮换 Secret
                            </UiButton>
                            <UiButton :disabled="busy === provider.provider_key" :loading="busy === provider.provider_key" @click="save(provider)">
                                保存配置
                            </UiButton>
                        </div>
                    </div>
                </div>
            </section>
        </div>

        <p class="mt-4 flex items-start gap-2 text-[10.5px] leading-5 text-muted-foreground">
            <ShieldCheck :size="13" class="mt-0.5 shrink-0" aria-hidden="true" />
            Secret 使用服务端主密钥加密保存；接口、日志和审计都不会返回明文。未配置完整的 Provider 不会出现在登录页。
        </p>
    </div>
</template>
