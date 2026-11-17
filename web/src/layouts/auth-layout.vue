<script setup lang="ts">
import { computed } from 'vue'
import ThemeSwitcher from '@/components/ui/theme-switcher/theme-switcher.vue'
import AuthVisual from '@/components/brand/auth-visual.vue'
import SiteLogo from '@/components/brand/site-logo.vue'
import SiteFooter from '@/components/brand/site-footer.vue'
import { useBrandingStore } from '@/pinia/stores/branding'
const props = defineProps<{ title: string; description: string; oidc?: boolean }>()
const brand = useBrandingStore()
const appearance = computed(() => props.oidc ? brand.current.oidc : brand.current.login)
</script>
<template>
    <main class="auth-layout">
        <AuthVisual :branding="brand.current.branding" :appearance="appearance" />
        <section class="auth-panel">
            <div class="auth-panel-top"><ThemeSwitcher /></div>
            <div class="auth-form">
                <div class="auth-mobile-brand"><SiteLogo :name="brand.current.branding.site_name" :url="brand.current.branding.logo_url" /></div>
                <div class="auth-eyebrow"><span class="status-dot" aria-hidden="true" />安全登录</div>
                <h1>{{ title }}</h1><p>{{ description }}</p><slot />
            </div>
            <SiteFooter :config="brand.current.branding.footer" />
        </section>
    </main>
</template>
