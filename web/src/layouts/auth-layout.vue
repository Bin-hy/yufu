<script setup lang="ts">
import { Fingerprint, KeyRound, ShieldCheck } from '@lucide/vue'
import ThemeSwitcher from '@/components/ui/theme-switcher/theme-switcher.vue'
import YufuMark from '@/components/brand/yufu-mark.vue'
defineProps<{ title: string; description: string }>()
const features = [
    { icon: Fingerprint, text: '一个账户，统一进入所有已授权应用' },
    { icon: KeyRound, text: '权限边界清晰，访问范围始终可见' },
    { icon: ShieldCheck, text: '每一次授权与撤销都留有审计记录' },
]
</script>
<template>
    <main class="auth-layout">
        <aside class="auth-visual" aria-label="yufu 身份与访问管理">
            <div class="auth-brand">
                <span class="auth-brand-mark"><YufuMark :size="26" /></span>
                <span>yu<span class="text-white/60">fu</span></span>
            </div>
            <div class="auth-visual-copy">
                <span class="auth-kicker">IDENTITY & ACCESS</span>
                <h2>一个账户，<br />进入每一个工作场景。</h2>
                <p>
                    统一管理企业账户、应用接入与授权边界。清晰的身份，是每一次安全协作的起点。
                </p>
                <ul class="auth-features">
                    <li v-for="feature in features" :key="feature.text">
                        <component :is="feature.icon" :size="13" aria-hidden="true" />
                        {{ feature.text }}
                    </li>
                </ul>
            </div>
            <p class="auth-visual-footer">
                <span class="h-px w-6 bg-white/20" aria-hidden="true" />
                企业统一身份认证 · yufu
            </p>
        </aside>
        <section class="auth-panel">
            <div class="auth-panel-top"><ThemeSwitcher /></div>
            <div class="auth-form">
                <div class="auth-mobile-brand">
                    <span class="brand-mark"><YufuMark :size="24" /></span>
                    <span>yu<span class="text-primary">fu</span></span>
                </div>
                <div class="auth-eyebrow"><span class="status-dot" aria-hidden="true" />安全登录</div>
                <h1>{{ title }}</h1>
                <p>{{ description }}</p>
                <slot />
            </div>
            <p class="auth-legal">yufu · 身份与访问管理 · 受企业安全策略保护</p>
        </section>
    </main>
</template>
