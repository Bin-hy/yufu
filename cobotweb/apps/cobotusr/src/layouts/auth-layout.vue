<script setup lang="ts">
import { Command, ShieldCheck, ArrowUpRight } from '@lucide/vue'
import ThemeSwitcher from '@/components/ui/theme-switcher/theme-switcher.vue'
defineProps<{ title: string; description: string }>()
</script>
<template>
    <main class="auth-layout min-h-screen grid lg:grid-cols-2">
        <aside
            class="hidden lg:flex bg-primary text-primary-foreground p-14 xl:p-20 flex-col justify-between"
        >
            <div class="flex items-center gap-3 font-semibold text-2xl">
                <Command :size="28" aria-hidden="true" />cobotusr
            </div>
            <div>
                <ShieldCheck :size="40" stroke-width="1.4" class="mb-7" aria-hidden="true" />
                <h2 class="text-4xl font-medium leading-tight tracking-tight">
                    让身份与权限<br />清晰有序。
                </h2>
                <p class="mt-6 text-sm leading-7 max-w-sm">
                    统一管理平台账户、角色、应用接入与授权，掌握每一次访问。
                </p>
            </div>
            <p class="text-xs flex items-center gap-2">
                账户 · 访问控制 · 应用<ArrowUpRight :size="14" aria-hidden="true" />
            </p>
        </aside>
        <div class="flex flex-col items-center justify-center px-5 py-10 sm:px-12 relative">
            <div class="absolute top-5 right-5"><ThemeSwitcher /></div>
            <div class="w-full max-w-sm">
                <div class="flex items-center gap-2 font-semibold text-primary mb-10 lg:hidden">
                    <Command :size="22" aria-hidden="true" />cobotusr
                </div>
                <h1 class="text-3xl font-semibold tracking-tight">{{ title }}</h1>
                <p class="mt-3 mb-8 text-sm text-muted-foreground leading-6">{{ description }}</p>
                <slot />
            </div>
            <p class="mt-10 text-xs text-muted-foreground">cobotusr · 身份与访问管理</p>
        </div>
    </main>
</template>
