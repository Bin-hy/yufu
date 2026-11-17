<script setup lang="ts">
import { ref, onScopeDispose } from 'vue'
import type { AssetKind, AssetMetadata } from '@/branding/types'
import { uploadSiteAsset } from '../api/site-settings'
import { sessionEpoch } from '@/workspace/session'
import UiButton from '@/components/ui/button/button.vue'
const props = defineProps<{ modelValue: string | null; kind: AssetKind; label: string; preview?: string; disabled?: boolean; maxBytes: number; inherited?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: string | null]; uploaded: [value: AssetMetadata]; busy: [value: boolean] }>()
const pending = ref(false), error = ref('')
const controller = new AbortController()
onScopeDispose(() => controller.abort())
async function select(event: Event) {
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]; input.value = ''
    if (!file || pending.value) return
    error.value = ''
    if (file.size > props.maxBytes) { error.value = `图片不能超过 ${props.maxBytes / 1024 / 1024} MiB`; return }
    pending.value = true; emit('busy', true)
    const epoch = sessionEpoch.value
    try {
        const metadata = await uploadSiteAsset(props.kind, file, controller.signal)
        if (controller.signal.aborted || epoch !== sessionEpoch.value) return
        emit('uploaded', metadata); emit('update:modelValue', metadata.id)
    } catch (e) { if (!controller.signal.aborted && epoch === sessionEpoch.value) error.value = e instanceof Error ? e.message : '上传失败，请重试' }
    finally { pending.value = false; emit('busy', false) }
}
</script>
<template>
    <div class="site-asset-field">
        <span class="text-xs font-medium">{{ label }}</span>
        <img v-if="preview" :src="preview" class="site-asset-thumb" :alt="label + '预览'" />
        <span v-else class="text-xs text-muted-foreground">{{ inherited ? '继承全站封面' : modelValue ? '正在加载图片预览' : '使用默认外观' }}</span>
        <label class="site-upload-label" :class="{ 'opacity-50': disabled || pending }">{{ pending ? '上传中…' : '上传图片' }}<input type="file" :aria-label="'上传' + label" :accept="kind === 'favicon' ? '.png,.jpg,.jpeg,.webp,.ico' : '.png,.jpg,.jpeg,.webp'" :disabled="disabled || pending" @change="select" /></label>
        <UiButton v-if="modelValue" size="sm" variant="outline" :disabled="disabled || pending" @click="emit('update:modelValue', null)">{{ inherited ? '清除覆盖' : '移除' }}</UiButton>
        <p class="form-help">PNG、JPEG、WebP{{ kind === 'favicon' ? '、ICO' : '' }} · 最大 {{ maxBytes / 1024 / 1024 }} MiB · 宽高不超过 8192 像素</p>
        <p v-if="error" class="field-error" role="alert">{{ error }}</p>
    </div>
</template>
