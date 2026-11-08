<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute } from 'vue-router'
import { Check } from '@lucide/vue'
import { getAuthorizeClientInfo, submitConsent } from './api'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import { useFormDraft } from '@/composables/use-form-draft'
import AuthLayout from '@/layouts/auth-layout.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const route = useRoute(),
    scalar = (key: string) =>
        typeof route.query[key] === 'string' ? (route.query[key] as string) : ''
const params = computed(() => ({
    client_id: scalar('client_id'),
    redirect_uri: scalar('redirect_uri'),
    response_type: scalar('response_type') || 'code',
    code_challenge: scalar('code_challenge') || undefined,
    code_challenge_method: scalar('code_challenge_method') || 'S256',
    scope: scalar('scope') || undefined,
    state: scalar('state') || undefined,
    nonce: scalar('nonce') || undefined,
}))
const state = useAsyncQuery({
        key: () => JSON.stringify(params.value),
        loader: (signal) => {
            if (!params.value.client_id) throw Error('缺少应用标识，请从应用重新发起授权')
            return getAuthorizeClientInfo(
                { client_id: params.value.client_id, scope: params.value.scope },
                { signal, errorPresentation: 'local' },
            )
        },
    }),
    { data, status, error } = state
const mutation = useAsyncMutation(),
    { pending, error: writeError } = mutation
useFormDraft('应用授权', () => null, { submitting: pending })
async function submit(approved: boolean) {
    await mutation.run(
        () => submitConsent({ ...params.value, approved }, { errorPresentation: 'local' }),
        (response) => {
            const url = new URL(response.redirect_url, window.location.origin)
            if (!['https:', 'http:'].includes(url.protocol))
                throw Error('应用返回的地址无效，请重新发起授权')
            window.location.assign(url.href)
        },
    )
}
watch(data, (info) => {
    if (info?.already_consented) void submit(true)
})
</script>
<template>
    <AuthLayout
        :title="data ? '授权 ' + data.client_name : '应用授权'"
        description="确认应用将获得的访问权限。"
        ><DataState :status="status" :error="error" @retry="state.refresh"
            ><div class="space-y-6">
                <p class="text-sm text-muted-foreground">
                    <strong class="text-foreground">{{ data?.client_name }}</strong>
                    请求访问你的账户。
                </p>
                <ul class="space-y-2" aria-label="请求的权限">
                    <li
                        v-for="scope in data?.scopes"
                        :key="scope.name"
                        class="flex gap-3 items-start rounded-lg bg-muted p-3"
                    >
                        <Check
                            :size="18"
                            class="text-primary shrink-0 mt-0.5"
                            aria-hidden="true"
                        /><span
                            ><span class="block text-sm font-medium">{{ scope.name }}</span
                            ><span class="block text-xs text-muted-foreground mt-1">{{
                                scope.description
                            }}</span></span
                        >
                    </li>
                </ul>
                <p v-if="writeError" class="inline-error" role="alert">{{ writeError }}</p>
                <div class="grid grid-cols-2 gap-3">
                    <UiButton :disabled="pending" variant="outline" @click="submit(false)"
                        >拒绝授权</UiButton
                    ><UiButton :disabled="pending" @click="submit(true)">{{
                        pending ? '正在处理…' : '同意授权'
                    }}</UiButton>
                </div>
                <p class="text-xs text-muted-foreground leading-5 break-all">
                    授权结果将返回：{{ params.redirect_uri }}
                </p>
            </div></DataState
        ></AuthLayout
    >
</template>
