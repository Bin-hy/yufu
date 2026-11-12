<script setup lang="ts">
import { ArrowLeft, Home, ShieldAlert, SearchX } from '@lucide/vue'
import { useRouter } from 'vue-router'
import { getToken } from '@/common/cache/cookies'
import UiButton from '@/components/ui/button/button.vue'
defineProps<{ status: 403 | 404 }>()
const router = useRouter()
function back() {
    const previous = router.options.history.state.back
    if (
        typeof previous === 'string' &&
        previous.startsWith('/') &&
        !previous.startsWith('//') &&
        router.resolve(previous).meta.workspace
    )
        router.back()
    else void router.replace(getToken() ? '/dashboard' : '/login')
}
</script>
<template>
    <main class="min-h-screen flex items-center justify-center px-6">
        <div class="max-w-md text-center">
            <component
                :is="status === 403 ? ShieldAlert : SearchX"
                :size="42"
                stroke-width="1.3"
                class="mx-auto mb-6 text-primary"
                aria-hidden="true"
            />
            <p class="text-xs text-muted-foreground tracking-widest mb-3">{{ status }}</p>
            <h1 class="text-3xl font-semibold tracking-tight">
                {{ status === 403 ? '无法访问此页面' : '页面不存在' }}
            </h1>
            <p class="text-sm text-muted-foreground leading-6 mt-4">
                {{
                    status === 403
                        ? '当前账户没有访问权限，请联系管理员。'
                        : '地址可能已经变更，你可以返回工作台继续。'
                }}
            </p>
            <div class="flex justify-center gap-3 mt-8">
                <UiButton variant="outline" @click="back"
                    ><ArrowLeft :size="16" aria-hidden="true" />返回上一页</UiButton
                ><RouterLink
                    :to="getToken() ? '/dashboard' : '/login'"
                    class="ui-button ui-button-default"
                    ><Home :size="16" aria-hidden="true" />{{
                        getToken() ? '返回工作台' : '返回登录'
                    }}</RouterLink
                >
            </div>
        </div>
    </main>
</template>
