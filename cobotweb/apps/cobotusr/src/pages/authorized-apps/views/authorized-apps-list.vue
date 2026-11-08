<script setup lang="ts">
import { computed, ref } from 'vue'
import { Blocks, RefreshCw } from '@lucide/vue'
import { listAuthorizedApps, revokeAuthorizedApp } from '../api'
import type { AuthorizedAppVO } from '../api/type'
import { useAsyncQuery } from '@/composables/use-async-query'
import { usePageContext } from '@/workspace/page-context'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import DataState from '@/components/workspace/data-state.vue'
import UiButton from '@/components/ui/button/button.vue'
const context = usePageContext(),
    state = useAsyncQuery({
        key: 'authorized-apps',
        active: context.active,
        loader: (signal) => listAuthorizedApps({ signal, errorPresentation: 'local' }),
    }),
    { data, status, error, refreshing } = state
const revoked = ref(new Set<number>())
const visibleApps = computed(
    () => data.value?.filter((app) => !revoked.value.has(app.consent_id)) ?? [],
)
function revoke(app: AuthorizedAppVO) {
    void showModal({
        title: '撤销应用授权？',
        content: '「' + app.client_name + '」的所有令牌将被吊销，之后需要重新授权。',
        destructive: true,
        confirmText: '撤销授权',
        confirmCallback: async () => {
            await revokeAuthorizedApp(app.consent_id, { errorPresentation: 'local' })
            revoked.value.add(app.consent_id)
            emitToast({ type: 'success', message: '已撤销授权' })
            await state.refresh()
        },
    })
}
</script>
<template>
    <section class="workspace-page">
        <PageHeader
            title="已授权应用"
            description="查看获准访问账户的应用，随时撤回访问权限。"
            eyebrow="APPLICATIONS"
            ><template #actions
                ><UiButton variant="outline" :disabled="refreshing" @click="state.refresh"
                    ><RefreshCw :size="15" aria-hidden="true" />刷新</UiButton
                ></template
            ></PageHeader
        >
        <div class="panel">
            <DataState
                :status="status"
                :error="error"
                :refreshing="refreshing"
                :empty="!visibleApps.length"
                empty-title="尚未授权任何应用"
                empty-description="授权应用后，可以在这里管理访问权限。"
                @retry="state.refresh"
                ><div class="divide-y divide-border">
                    <article
                        v-for="app in visibleApps"
                        :key="app.consent_id"
                        class="flex flex-col sm:flex-row gap-4 p-5 sm:items-center"
                    >
                        <div
                            class="bg-secondary text-primary w-11 h-11 rounded-lg flex items-center justify-center shrink-0"
                        >
                            <Blocks :size="21" aria-hidden="true" />
                        </div>
                        <div class="flex-1 min-w-0">
                            <h2 class="font-semibold">{{ app.client_name }}</h2>
                            <p class="text-xs text-muted-foreground mt-1">
                                授权于 {{ app.authorized_at }}
                            </p>
                            <div class="flex flex-wrap gap-1.5 mt-3">
                                <span
                                    v-for="scope in app.scope.split(' ').filter(Boolean)"
                                    :key="scope"
                                    class="status-badge"
                                    >{{ scope }}</span
                                >
                            </div>
                        </div>
                        <UiButton variant="outline" class="text-destructive" @click="revoke(app)"
                            >撤销</UiButton
                        >
                    </article>
                </div></DataState
            >
        </div>
    </section>
</template>
