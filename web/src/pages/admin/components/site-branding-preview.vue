<script setup lang="ts">
import { ref } from 'vue'
import type { PublicSiteBranding } from '@/branding/types'
import AuthVisual from '@/components/brand/auth-visual.vue'
import SiteLogo from '@/components/brand/site-logo.vue'
import SiteFooter from '@/components/brand/site-footer.vue'
defineProps<{ value: PublicSiteBranding }>()
const oidc = ref(false)
</script>
<template>
    <section class="site-preview" aria-label="站点外观预览">
        <div class="site-preview-bar"><strong>外观预览</strong><label><input v-model="oidc" type="checkbox" /> OIDC 登录与授权页</label></div>
        <div class="site-preview-body">
            <AuthVisual :branding="value.branding" :appearance="oidc ? value.oidc : value.login" />
            <div class="site-preview-panel"><SiteLogo :name="value.branding.site_name" :url="value.branding.logo_url" /><h3>{{ oidc ? '登录以继续授权' : '欢迎回来' }}</h3><p>用户名</p><div class="site-preview-input" /><p>密码</p><div class="site-preview-input" /><div class="site-preview-submit" aria-hidden="true">登录</div><SiteFooter :config="value.branding.footer" /></div>
        </div>
        <p class="form-help p-3">预览仅展示外观，修改保存后才对其他用户生效。窄屏认证页面沿用移动端布局。</p>
    </section>
</template>
