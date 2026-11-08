<script setup lang="ts">
import { ref } from 'vue'
import { Copy, Eye, EyeOff } from '@lucide/vue'
import UiButton from '@/components/ui/button/button.vue'
const props = defineProps<{ secret: string; clientId?: string }>(),
    visible = ref(false),
    copied = ref(false),
    error = ref('')
async function copy() {
    try {
        await navigator.clipboard.writeText(props.secret)
        copied.value = true
        error.value = ''
    } catch {
        error.value = '复制失败，请显示密钥后手动复制。'
    }
}
</script>
<template>
    <div class="rounded-lg border border-primary/30 bg-primary/5 p-4 space-y-3">
        <p class="font-medium">保存客户端密钥</p>
        <p class="form-help">密钥仅在本次结果中显示，刷新或关闭页面后会丢失。请先保存到可信的位置。</p>
        <p v-if="clientId" class="text-xs break-all">Client ID：{{ clientId }}</p>
        <div class="flex items-center gap-2">
            <code class="flex-1 min-w-0 break-all text-xs select-all">{{
                visible ? secret : '••••••••••••••••••••••••'
            }}</code
            ><UiButton
                size="icon"
                variant="ghost"
                :aria-label="visible ? '隐藏密钥' : '显示密钥'"
                :aria-pressed="visible"
                @click="visible = !visible"
                ><EyeOff v-if="visible" :size="16" aria-hidden="true" /><Eye
                    v-else
                    :size="16"
                    aria-hidden="true" /></UiButton
            ><UiButton variant="outline" size="sm" @click="copy"
                ><Copy :size="14" aria-hidden="true" />{{ copied ? '已复制' : '复制密钥' }}</UiButton
            >
        </div>
        <p v-if="copied" role="status" class="text-xs text-primary">密钥已复制。</p>
        <p v-if="error" role="alert" class="text-xs text-destructive">{{ error }}</p>
    </div>
</template>
