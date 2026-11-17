<script setup lang="ts">
import { computed } from 'vue'
import type { FooterConfig } from '@/branding/types'
import { safeLink } from '@/branding/resolve'
const props = defineProps<{ config: FooterConfig }>()
const visible = computed(() => props.config.copyright_text || props.config.filing_number || props.config.links.length)
</script>
<template>
    <footer v-if="visible" class="site-footer">
        <span v-if="config.copyright_text">{{ config.copyright_text }}</span>
        <a v-if="config.filing_number && safeLink(config.filing_url)" :href="config.filing_url!" target="_blank" rel="noopener noreferrer">{{ config.filing_number }}</a>
        <span v-else-if="config.filing_number">{{ config.filing_number }}</span>
        <template v-for="(link, index) in config.links" :key="index"><a v-if="safeLink(link.url) && link.label" :href="link.url" target="_blank" rel="noopener noreferrer">{{ link.label }}</a></template>
    </footer>
</template>
