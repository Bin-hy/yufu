import { defineStore } from 'pinia'
import { ref } from 'vue'
import { defaultDocument } from '@/branding/defaults'
import { resolveBranding } from '@/branding/resolve'
import { loadPublicBranding } from '@/branding/api'
import type { PublicSiteBranding } from '@/branding/types'
export const useBrandingStore = defineStore('branding', () => {
    const current = ref(resolveBranding(defaultDocument()))
    let generation = 0
    let title = '工作台'
    let favicon: HTMLLinkElement | null = null
    function updateDocument() {
        document.title = `${title} · ${current.value.branding.site_name}`
        const url = current.value.branding.favicon_url ?? '/favicon.svg'
        favicon ??= document.querySelector<HTMLLinkElement>('link[rel="icon"]')
        if (!favicon) { favicon = document.createElement('link'); favicon.rel = 'icon'; document.head.append(favicon) }
        favicon.removeAttribute('type')
        favicon.onerror = () => { if (favicon) { favicon.onerror = null; favicon.href = '/favicon.svg' } }
        favicon.href = url
    }
    function setPublished(value: PublicSiteBranding) { generation++; current.value = value; updateDocument() }
    function setPageTitle(value: string) { title = value; updateDocument() }
    async function refresh() {
        const request = ++generation
        try {
            const value = await loadPublicBranding(AbortSignal.timeout(5000))
            if (request === generation) { current.value = value; updateDocument() }
        } catch {
            if (request === generation) { current.value = resolveBranding(defaultDocument()); updateDocument() }
        }
    }
    return { current, refresh, setPublished, setPageTitle }
})
