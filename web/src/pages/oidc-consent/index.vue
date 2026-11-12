<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute } from 'vue-router'
import { AtSign, Check, ShieldCheck, UserRound, X } from '@lucide/vue'
import { inspectFlow, completeFlow } from './api'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import AuthLayout from '@/layouts/auth-layout.vue'
import LoginForm from '@/components/workspace/login-form.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const route = useRoute()
const flowId = computed(() => (typeof route.query.flow_id === 'string' ? route.query.flow_id : ''))
const state = useAsyncQuery({
    key: () => flowId.value,
    loader: (signal) => {
        if (!flowId.value) throw new Error('授权流程缺失，请从应用重新发起登录')
        return inspectFlow(flowId.value, signal)
    },
})
const { data, status, error } = state
const mutation = useAsyncMutation()
const { pending, error: writeError } = mutation
const scopeDetails: Record<string, { label: string; description: string; icon: unknown }> = {
    openid: { label: '账户身份', description: '确认你的统一账户身份', icon: UserRound },
    profile: { label: '用户名与显示名称', description: '读取用户名和显示名称', icon: UserRound },
    email: { label: '邮箱与验证状态', description: '读取邮箱及验证状态', icon: AtSign },
}
const scopes = computed(() =>
    (data.value?.scopes ?? '')
        .split(' ')
        .filter(Boolean)
        .map((scope) => ({ scope, ...(scopeDetails[scope] ?? { label: scope, description: scope, icon: ShieldCheck }) })),
)
const initial = computed(() => (data.value?.application_name ?? 'A').slice(0, 1).toUpperCase())
async function submit(approved: boolean) {
    if (pending.value) return
    await mutation.run(
        () => completeFlow(flowId.value, approved),
        (response) => {
            const url = new URL(response.redirect_url)
            if (!['http:', 'https:'].includes(url.protocol)) throw new Error('授权返回地址无效')
            window.location.assign(url.href)
        },
    )
}
watch(data, (value) => {
    if (value?.interaction === 'ready') void submit(true)
    if (value?.interaction === 'denied') void submit(false)
})
</script>
<template>
    <LoginForm v-if="data?.interaction === 'login'" oidc :flow-id="flowId" />
    <AuthLayout
        v-else
        :title="data ? '授权 ' + data.application_name : '应用授权'"
        description="企业统一身份认证"
    >
        <DataState :status="status" :error="error" :skeleton-rows="2" @retry="state.refresh()">
            <div class="space-y-5">
                <div class="flex items-center gap-3 rounded-xl border border-border bg-muted/35 p-4">
                    <span class="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-primary text-base font-semibold text-primary-foreground">
                        {{ initial }}
                    </span>
                    <div class="min-w-0">
                        <p class="truncate text-sm font-semibold">{{ data?.application_name }}</p>
                        <p class="mt-0.5 flex items-center gap-1 text-[11px] text-muted-foreground">
                            <ShieldCheck :size="11" aria-hidden="true" />已通过企业身份验证
                        </p>
                    </div>
                </div>

                <div>
                    <p class="text-xs leading-5 text-muted-foreground">
                        当前账户 <strong class="text-foreground">{{ data?.username }}</strong> 将向该应用提供：
                    </p>
                    <ul class="mt-3 space-y-2">
                        <li
                            v-for="scope in scopes"
                            :key="scope.scope"
                            class="flex items-start gap-3 rounded-lg border border-border bg-card p-3"
                        >
                            <span class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-secondary text-secondary-foreground">
                                <component :is="scope.icon" :size="14" aria-hidden="true" />
                            </span>
                            <span class="min-w-0">
                                <span class="block text-xs font-semibold">{{ scope.label }}</span>
                                <span class="mt-0.5 block text-[10.5px] text-muted-foreground">{{ scope.description }}</span>
                            </span>
                            <Check :size="14" class="ml-auto mt-1 shrink-0 text-success" aria-hidden="true" />
                        </li>
                    </ul>
                </div>

                <p v-if="writeError" role="alert" class="form-alert">{{ writeError }}</p>
                <p class="text-[10.5px] leading-5 text-muted-foreground">
                    授权后你可以随时在「账户中心 → 资料授权」中撤回。拒绝授权不会影响其他应用。
                </p>
                <div class="grid grid-cols-2 gap-3">
                    <UiButton variant="outline" :disabled="pending" @click="submit(false)">
                        <X :size="14" aria-hidden="true" />拒绝授权
                    </UiButton>
                    <UiButton :disabled="pending" :loading="pending" @click="submit(true)">
                        <Check v-if="!pending" :size="14" aria-hidden="true" />同意授权
                    </UiButton>
                </div>
            </div>
        </DataState>
    </AuthLayout>
</template>
