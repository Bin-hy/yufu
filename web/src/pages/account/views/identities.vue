<script setup lang="ts">
defineOptions({ name: 'AccountIdentities' })
import { computed, onMounted, onScopeDispose, ref } from 'vue'
import { KeyRound, Link2, ShieldCheck, Unlink } from '@lucide/vue'
import ExternalProviderIcon from '@/components/brand/external-provider-icon.vue'
import InfoHint from '@/components/workspace/info-hint.vue'
import {
    getExternalIdentities,
    getExternalProviders,
    startExternalBind,
    unbindExternalIdentity,
    type ExternalIdentity,
    type ExternalProvider,
} from '../api/identities'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import UiButton from '@/components/ui/button/button.vue'

const identities = ref<ExternalIdentity[]>([])
const providers = ref<ExternalProvider[]>([])
const loading = ref(true)
const error = ref('')
const pending = ref('')
const controller = new AbortController()

const bindable = computed(() =>
    providers.value.filter(provider => provider.mode === 'oauth' && !identities.value.some(identity => identity.provider_key === provider.key)),
)
const nativeProviders = computed(() => providers.value.filter(provider => provider.mode === 'native'))

async function load() {
    loading.value = true
    error.value = ''
    try {
        const [bound, available] = await Promise.all([
            getExternalIdentities(controller.signal),
            getExternalProviders(controller.signal),
        ])
        identities.value = bound
        providers.value = available
    } catch (e) {
        if (!controller.signal.aborted) error.value = e instanceof Error ? e.message : '无法读取第三方身份'
    } finally {
        loading.value = false
    }
}

onMounted(load)
onScopeDispose(() => controller.abort())

async function bind(provider: ExternalProvider) {
    if (pending.value) return
    const confirmed = await showModal({
        title: `绑定 ${provider.label}？`,
        content: `将跳转到 ${provider.label} 完成授权。若该平台已登录并授权过 Yufu，可能直接返回，不再显示授权页面。`,
        confirmText: '继续授权',
        cancelText: '取消',
    })
    if (!confirmed) return
    pending.value = provider.key
    try {
        const result = await startExternalBind(provider.key)
        window.location.assign(result.authorize_url)
    } catch (e) {
        emitToast({ type: 'error', message: e instanceof Error ? e.message : '无法发起绑定' })
    } finally {
        pending.value = ''
    }
}

async function unbind(identity: ExternalIdentity) {
    if (pending.value) return
    const confirmed = await showModal({
        title: `解绑 ${identity.provider_label}？`,
        content: '解绑后该第三方账号将无法登录 Yufu。请确认你仍可通过密码或其他已绑定身份登录。',
        confirmText: '解绑',
        cancelText: '取消',
        destructive: true,
    })
    if (!confirmed) return
    pending.value = identity.id
    try {
        await unbindExternalIdentity(identity.id)
        emitToast({ type: 'success', message: '第三方身份已解绑' })
        await load()
    } catch (e) {
        emitToast({ type: 'error', message: e instanceof Error ? e.message : '解绑失败' })
    } finally {
        pending.value = ''
    }
}

function date(value: number | null) {
    return value ? new Date(value * 1000).toLocaleString('zh-CN') : '从未登录'
}
</script>

<template>
    <div class="workspace-page">
        <PageHeader
            title="第三方身份"
            eyebrow="IDENTITIES"
            hint="第三方账号只用于认证身份；下游应用仍然只看到 Yufu 统一身份。"
        />

        <p v-if="error" role="alert" class="form-alert mb-4">{{ error }}</p>

        <div class="content-grid content-grid-sidebar">
            <div class="section-stack">
                <section class="panel">
                    <header class="panel-heading">
                        <h2 class="flex items-center gap-2">
                            已绑定身份
                            <InfoHint text="每个第三方身份只能绑定一个 Yufu 账户。" />
                        </h2>
                        <span class="status-badge status-brand">{{ identities.length }}</span>
                    </header>
                    <div v-if="loading" class="surface-body text-sm text-muted-foreground">正在读取…</div>
                    <div v-else-if="!identities.length" class="surface-body text-sm text-muted-foreground">
                        暂无绑定身份
                    </div>
                    <ul v-else class="divide-y divide-border">
                        <li
                            v-for="identity in identities"
                            :key="identity.id"
                            class="flex flex-wrap items-center gap-4 px-5 py-4"
                        >
                            <span class="flex h-10 w-10 items-center justify-center rounded-xl border border-border bg-background">
                                <ExternalProviderIcon :provider="identity.provider_key" :size="18" />
                            </span>
                            <div class="min-w-0 flex-1">
                                <p class="flex items-center gap-2 text-sm font-semibold">
                                    {{ identity.provider_label }}
                                    <span class="status-badge status-success">已绑定</span>
                                </p>
                                <p class="mt-0.5 truncate text-[11.5px] text-muted-foreground">
                                    {{ identity.display_name || '未提供显示名称' }} · {{ identity.subject_hint }}
                                </p>
                                <p class="mt-1 text-[10.5px] text-muted-foreground">
                                    绑定于 {{ date(identity.created_at) }} · 最近登录 {{ date(identity.last_login_at) }}
                                </p>
                            </div>
                            <UiButton
                                variant="outline"
                                :loading="pending === identity.id"
                                :disabled="!!pending"
                                @click="unbind(identity)"
                            >
                                <Unlink :size="14" aria-hidden="true" />解绑
                            </UiButton>
                        </li>
                    </ul>
                </section>

                <section v-if="bindable.length || nativeProviders.length" class="panel">
                    <header class="panel-heading">
                        <h2 class="flex items-center gap-2">
                            绑定新身份
                            <InfoHint text="绑定过程会跳转到对应平台完成授权，然后返回 Yufu。" />
                        </h2>
                        <ShieldCheck :size="18" class="text-muted-foreground" aria-hidden="true" />
                    </header>
                    <div class="surface-body space-y-4">
                        <div v-if="bindable.length" class="grid gap-3 sm:grid-cols-2">
                            <UiButton
                                v-for="provider in bindable"
                                :key="provider.key"
                                variant="outline"
                                block
                                class="justify-start gap-2"
                                :loading="pending === provider.key"
                                :disabled="!!pending"
                                @click="bind(provider)"
                            >
                                <ExternalProviderIcon :provider="provider.key" :size="16" />
                                绑定 {{ provider.label }}
                            </UiButton>
                        </div>
                        <p v-if="nativeProviders.length" class="text-[11px] leading-5 text-muted-foreground">
                            微信身份请在小程序中绑定。
                        </p>
                    </div>
                </section>
            </div>

            <aside class="section-stack sticky-column">
                <section class="surface p-5">
                    <p class="flex items-center gap-2 text-xs font-semibold">
                        <ShieldCheck :size="15" class="text-success" aria-hidden="true" />安全提示
                    </p>
                    <ul class="mt-3 space-y-2 text-[11px] leading-5 text-muted-foreground">
                        <li class="flex items-center gap-2"><ShieldCheck :size="12" class="shrink-0" aria-hidden="true" />仅证明身份，不授予管理权限</li>
                        <li class="flex items-center gap-2"><Link2 :size="12" class="shrink-0" aria-hidden="true" />绑定必须由已登录用户主动完成</li>
                        <li class="flex items-center gap-2"><KeyRound :size="12" class="shrink-0" aria-hidden="true" />保留至少一个可用登录方式</li>
                    </ul>
                </section>
            </aside>
        </div>
    </div>
</template>
