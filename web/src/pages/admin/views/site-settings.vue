<script setup lang="ts">
defineOptions({ name: 'SiteSettings' })
import { computed, ref, onScopeDispose, watch } from 'vue'
import type { SiteSettingsDocument, SettingsResponse, AssetMetadata } from '@/branding/types'
import { resolveBranding } from '@/branding/resolve'
import { getSiteSettings, saveSiteSettings, resetSiteSettings, readSiteAsset } from '../api/site-settings'
import { useBrandingStore } from '@/pinia/stores/branding'
import { useFormDraft } from '@/composables/use-form-draft'
import { onSessionReset, sessionEpoch } from '@/workspace/session'
import { showModal } from '@/utils/modal'
import { emitToast } from '@/utils/toast'
import PageHeader from '@/components/workspace/page-header.vue'
import UiInput from '@/components/ui/input/input.vue'
import UiTextarea from '@/components/ui/textarea/textarea.vue'
import UiButton from '@/components/ui/button/button.vue'
import UiFormField from '@/components/ui/form-field/form-field.vue'
import SiteAssetField from '../components/site-asset-field.vue'
import SiteBrandingPreview from '../components/site-branding-preview.vue'
const brand = useBrandingStore()
const loaded = ref<SettingsResponse | null>(null), draft = ref<SiteSettingsDocument | null>(null)
const pending = ref(false), loading = ref(false), error = ref(''), conflict = ref(false)
const uploading = ref(new Set<string>()), previews = ref<Record<string, string>>({})
const previewFailures = ref(new Set<string>())
const controller = new AbortController()
let generation = 0
const previewLoads = new Set<string>()
const busy = computed(() => pending.value || uploading.value.size > 0)
const form = useFormDraft('站点设置', () => draft.value, { enabled: () => !!loaded.value, submitting: busy, discard: () => { draft.value = loaded.value ? JSON.parse(JSON.stringify(loaded.value.settings.document)) : null } })
const preview = computed(() => draft.value ? resolveBranding(draft.value, loaded.value?.settings.revision ?? 0, id => previews.value[id] ?? '') : null)
function resetLocal() {
    generation++; controller.abort()
    Object.values(previews.value).forEach(url => URL.revokeObjectURL(url))
    previews.value = {}; draft.value = null; loaded.value = null; form.suspend()
}
const stopReset = onSessionReset(resetLocal)
onScopeDispose(() => { stopReset(); resetLocal() })
async function assetPreview(id: string) {
    if (previews.value[id] || previewFailures.value.has(id)) return
    const epoch = sessionEpoch.value, request = generation, key = `${generation}:${id}`
    if (previewLoads.has(key)) return
    previewLoads.add(key)
    try {
        const blob = await readSiteAsset(id, controller.signal)
        if (controller.signal.aborted || epoch !== sessionEpoch.value || request !== generation) return
        const d = draft.value
        const active = d && [d.branding.logo_asset_id, d.branding.favicon_asset_id, d.branding.cover_asset_id, d.oidc.cover_asset_id].includes(id)
        if (active && !previews.value[id]) previews.value[id] = URL.createObjectURL(blob)
    } catch { if (epoch === sessionEpoch.value && request === generation) previewFailures.value.add(id) }
    finally { previewLoads.delete(key) }
}
watch(() => draft.value ? [draft.value.branding.logo_asset_id, draft.value.branding.favicon_asset_id, draft.value.branding.cover_asset_id, draft.value.oidc.cover_asset_id] : [], ids => {
    const active = new Set(ids.filter(Boolean))
    for (const [id, url] of Object.entries(previews.value)) if (!active.has(id)) { URL.revokeObjectURL(url); delete previews.value[id] }
    for (const id of active) void assetPreview(id)
})
function received(metadata: AssetMetadata) { previewFailures.value.delete(metadata.id); void assetPreview(metadata.id) }
function setBusy(key: string, value: boolean) { const next = new Set(uploading.value); if (value) next.add(key); else next.delete(key); uploading.value = next }
function apply(result: SettingsResponse) {
    loaded.value = result; draft.value = structuredClone(result.settings.document)
    form.markSaved(); error.value = ''; conflict.value = false
}
async function load() {
    if (loading.value || busy.value || controller.signal.aborted) return
    const request = ++generation, epoch = sessionEpoch.value
    loading.value = true; error.value = ''; previewFailures.value = new Set()
    try { const result = await getSiteSettings(controller.signal); if (request === generation && epoch === sessionEpoch.value) apply(result) }
    catch (e) { if (!controller.signal.aborted && request === generation) error.value = e instanceof Error ? e.message : '无法读取站点设置' }
    finally { loading.value = false }
}
async function reload() { if (await form.confirmDiscard()) await load() }
async function save(reset = false) {
    if (!draft.value || !loaded.value || busy.value || conflict.value) return
    if (reset && !(await showModal({ title: '恢复默认站点外观？', content: '网站品牌、页脚和 OIDC 外观覆盖将恢复默认。', confirmText: '恢复默认', cancelText: '取消', destructive: true }))) return
    const epoch = sessionEpoch.value
    pending.value = true; error.value = ''
    try {
        const result = reset ? await resetSiteSettings(loaded.value.settings.revision) : await saveSiteSettings(draft.value, loaded.value.settings.revision)
        if (controller.signal.aborted || epoch !== sessionEpoch.value) return
        apply(result); brand.setPublished(resolveBranding(result.settings.document, result.settings.revision))
        emitToast({ type: 'success', message: reset ? '已恢复默认外观' : '站点设置已保存' })
    } catch (e) {
        if (controller.signal.aborted || epoch !== sessionEpoch.value) return
        conflict.value = (e as { response?: { status?: number } })?.response?.status === 409
        error.value = conflict.value ? '设置已被其他操作更新。你的草稿已保留，请重新加载当前配置后再修改。' : e instanceof Error ? e.message : '保存失败，请稍后重试'
    } finally { pending.value = false }
}
void load()
</script>
<template>
    <div class="site-settings-page">
        <PageHeader title="站点设置" hint="维护网站品牌、认证页面与页脚；仅超级管理员可修改。" />
        <p v-if="error" class="form-alert" role="alert">{{ error }}</p>
        <UiButton v-if="!loaded || conflict" variant="outline" :disabled="loading || busy" @click="reload">{{ loading ? '正在读取…' : '重新加载当前配置' }}</UiButton>
        <template v-if="draft && loaded">
            <p v-if="previewFailures.size" class="form-help" role="status">部分图片预览无法加载。可重新加载设置后重试；已保存的图片配置未改变。</p>
            <p class="form-help">当前版本 {{ loaded.settings.revision }} · {{ form.dirty.value ? '有未保存的修改' : '已保存' }}。保存后其他用户下次打开或刷新页面生效。</p>
            <form class="site-settings-grid" @submit.prevent="save(false)">
                <div class="site-settings-fields">
                    <fieldset :disabled="busy || loading" class="site-settings-section"><legend>全站品牌</legend>
                        <UiFormField id="site-name" label="网站名称" required><UiInput id="site-name" v-model="draft.branding.site_name" :maxlength="loaded.limits.site_name" required /></UiFormField>
                        <UiFormField id="site-description" label="网站简介"><UiTextarea id="site-description" v-model="draft.branding.site_description" :maxlength="loaded.limits.site_description" /></UiFormField>
                        <SiteAssetField v-model="draft.branding.logo_asset_id" kind="logo" label="网站 Logo" :max-bytes="loaded.limits.logo_bytes" :preview="previews[draft.branding.logo_asset_id ?? '']" :disabled="busy" @uploaded="received" @busy="setBusy('logo', $event)" />
                        <SiteAssetField v-model="draft.branding.favicon_asset_id" kind="favicon" label="浏览器图标" :max-bytes="loaded.limits.favicon_bytes" :preview="previews[draft.branding.favicon_asset_id ?? '']" :disabled="busy" @uploaded="received" @busy="setBusy('favicon', $event)" />
                    </fieldset>
                    <fieldset :disabled="busy || loading" class="site-settings-section"><legend>普通登录页</legend>
                        <SiteAssetField v-model="draft.branding.cover_asset_id" kind="cover" label="全站认证封面" :max-bytes="loaded.limits.cover_bytes" :preview="previews[draft.branding.cover_asset_id ?? '']" :disabled="busy" @uploaded="received" @busy="setBusy('cover', $event)" />
                        <UiFormField id="welcome-title" label="欢迎标题"><UiInput id="welcome-title" v-model="draft.branding.welcome_title" :maxlength="loaded.limits.welcome_title" /></UiFormField>
                        <UiFormField id="welcome-description" label="欢迎说明"><UiTextarea id="welcome-description" v-model="draft.branding.welcome_description" :maxlength="loaded.limits.welcome_description" /></UiFormField>
                    </fieldset>
                    <fieldset :disabled="busy || loading" class="site-settings-section"><legend>OIDC 登录与授权页</legend><p class="form-help">默认继承全站品牌；留空则隐藏。</p>
                        <SiteAssetField v-model="draft.oidc.cover_asset_id" kind="cover" label="OIDC 独立封面" :max-bytes="loaded.limits.cover_bytes" :preview="previews[draft.oidc.cover_asset_id ?? draft.branding.cover_asset_id ?? '']" :inherited="true" :disabled="busy" @uploaded="received" @busy="setBusy('oidc-cover', $event)" />
                        <label class="checkbox-field"><input type="checkbox" :checked="draft.oidc.welcome_title !== null" @change="draft.oidc.welcome_title = ($event.target as HTMLInputElement).checked ? draft.branding.welcome_title : null" />自定义 OIDC 欢迎标题</label>
                        <UiInput v-if="draft.oidc.welcome_title !== null" v-model="draft.oidc.welcome_title" aria-label="OIDC 欢迎标题" :maxlength="loaded.limits.welcome_title" />
                        <label class="checkbox-field"><input type="checkbox" :checked="draft.oidc.welcome_description !== null" @change="draft.oidc.welcome_description = ($event.target as HTMLInputElement).checked ? draft.branding.welcome_description : null" />自定义 OIDC 欢迎说明</label>
                        <UiTextarea v-if="draft.oidc.welcome_description !== null" v-model="draft.oidc.welcome_description" aria-label="OIDC 欢迎说明" :maxlength="loaded.limits.welcome_description" />
                    </fieldset>
                    <fieldset :disabled="busy || loading" class="site-settings-section"><legend>页脚信息</legend>
                        <UiFormField id="copyright-text" label="版权文字"><UiTextarea id="copyright-text" v-model="draft.branding.footer.copyright_text" :maxlength="loaded.limits.copyright_text" /></UiFormField>
                        <UiFormField id="filing-number" label="备案号"><UiInput id="filing-number" v-model="draft.branding.footer.filing_number" :maxlength="loaded.limits.filing_number" /></UiFormField>
                        <UiFormField id="filing-url" label="备案链接"><UiInput id="filing-url" :model-value="draft.branding.footer.filing_url ?? ''" :maxlength="loaded.limits.link_url" placeholder="https://" @update:model-value="draft.branding.footer.filing_url = $event ? String($event) : null" /></UiFormField>
                        <div v-for="(link, index) in draft.branding.footer.links" :key="index" class="site-footer-link-editor"><UiInput v-model="link.label" :aria-label="'链接名称 ' + (index + 1)" placeholder="链接名称" :maxlength="loaded.limits.link_label" required /><UiInput v-model="link.url" :aria-label="'链接地址 ' + (index + 1)" placeholder="https://" :maxlength="loaded.limits.link_url" required /><UiButton variant="ghost" size="sm" @click="draft.branding.footer.links.splice(index, 1)">移除链接</UiButton></div>
                        <UiButton variant="outline" size="sm" :disabled="draft.branding.footer.links.length >= loaded.limits.links" @click="draft.branding.footer.links.push({ label: '', url: '' })">添加页脚链接</UiButton>
                    </fieldset>
                    <p class="form-help">未使用图片保留 24 小时；过期草稿需重新上传。</p>
                    <div class="site-settings-actions"><UiButton type="submit" :loading="pending" :disabled="busy || loading || conflict">保存设置</UiButton><UiButton variant="outline" :disabled="busy || loading || conflict" @click="save(true)">恢复默认</UiButton><UiButton variant="ghost" :disabled="busy || loading" @click="reload">重新加载</UiButton></div>
                </div>
                <SiteBrandingPreview v-if="preview" :value="preview" />
            </form>
        </template>
    </div>
</template>
