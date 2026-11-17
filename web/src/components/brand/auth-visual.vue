<script setup lang="ts">
import { ref, watch } from 'vue'
import type { AuthAppearance, PublicSiteBranding } from '@/branding/types'
import SiteLogo from './site-logo.vue'
const props = defineProps<{ branding: PublicSiteBranding['branding']; appearance: AuthAppearance }>()
const failed = ref(false)
watch(() => props.appearance.cover_url, () => { failed.value = false })
</script>
<template>
    <aside class="auth-visual" :aria-label="branding.site_name + ' 身份与访问管理'">
        <img v-if="appearance.cover_url && !failed" class="site-auth-cover" :src="appearance.cover_url" alt="" @error="failed = true" />
        <div class="auth-brand"><SiteLogo :name="branding.site_name" :url="branding.logo_url" /></div>
        <div class="auth-visual-copy">
            <span v-if="branding.site_description" class="auth-kicker">{{ branding.site_description }}</span>
            <h2 v-if="appearance.welcome_title">{{ appearance.welcome_title }}</h2>
            <p v-if="appearance.welcome_description">{{ appearance.welcome_description }}</p>
        </div>
        <p class="auth-visual-footer">{{ branding.site_name }}</p>
    </aside>
</template>
