<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRoute } from 'vue-router'
import { inspectFlow, completeFlow } from './api'
import { useAsyncQuery } from '@/composables/use-async-query'
import { useAsyncMutation } from '@/composables/use-async-mutation'
import AuthLayout from '@/layouts/auth-layout.vue'
import LoginForm from '@/components/workspace/login-form.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const route = useRoute()
const flowId = computed(() => typeof route.query.flow_id === 'string' ? route.query.flow_id : '')
const state = useAsyncQuery({ key: () => flowId.value, loader: (signal) => {
    if (!flowId.value) throw new Error('授权流程缺失，请从应用重新发起登录')
    return inspectFlow(flowId.value, signal)
} })
const { data, status, error } = state
const mutation = useAsyncMutation()
const { pending, error: writeError } = mutation
const descriptions: Record<string, string> = { openid: '确认你的统一账户身份', profile: '读取用户名和显示名称', email: '读取邮箱及验证状态' }
async function submit(approved: boolean) {
    if (pending.value) return
    await mutation.run(() => completeFlow(flowId.value, approved), response => {
        const url = new URL(response.redirect_url)
        if (!['http:', 'https:'].includes(url.protocol)) throw new Error('授权返回地址无效')
        window.location.assign(url.href)
    })
}
watch(data, value => {
    if (value?.interaction === 'ready') void submit(true)
    if (value?.interaction === 'denied') void submit(false)
})
</script>
<template>
    <LoginForm v-if="data?.interaction === 'login'" oidc :flow-id="flowId" />
    <AuthLayout v-else :title="data ? '登录 ' + data.application_name : '应用授权'" description="企业统一身份认证">
        <DataState :status="status" :error="error" @retry="state.refresh">
            <div class="space-y-5">
                <p>当前账户：<strong>{{ data?.username }}</strong></p>
                <p class="text-sm text-muted-foreground">{{ data?.application_name }} 将获得以下账户资料：</p>
                <ul class="space-y-2"><li v-for="scope in data?.scopes.split(' ')" :key="scope" class="rounded-lg bg-muted p-3 text-sm">{{ descriptions[scope] || scope }}</li></ul>
                <p v-if="writeError" role="alert" class="inline-error">{{ writeError }}</p>
                <div class="grid grid-cols-2 gap-3"><UiButton variant="outline" :disabled="pending" @click="submit(false)">拒绝授权</UiButton><UiButton :disabled="pending" @click="submit(true)">{{ pending ? '正在处理…' : '同意授权' }}</UiButton></div>
            </div>
        </DataState>
    </AuthLayout>
</template>
