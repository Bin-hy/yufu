<script setup lang="ts">
import SiteLogo from '@/components/brand/site-logo.vue'
import { useBrandingStore } from '@/pinia/stores/branding'
defineProps<{ collapsed?: boolean }>()
const brand = useBrandingStore()
</script>
<template><router-link to="/dashboard" class="brand" :class="{ '!justify-center !px-0': collapsed }" :aria-label="brand.current.branding.site_name + ' 工作台'"><SiteLogo :name="brand.current.branding.site_name" :url="brand.current.branding.logo_url" :collapsed="collapsed" /></router-link></template>
