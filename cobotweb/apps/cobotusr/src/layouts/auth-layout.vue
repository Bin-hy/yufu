<script setup lang="ts">
import { Boxes, Fingerprint, KeyRound, ShieldCheck } from '@lucide/vue'
import ThemeSwitcher from '@/components/ui/theme-switcher/theme-switcher.vue'
defineProps<{ title: string; description: string }>()
const features = [
    { icon: Fingerprint, text: '一个账户，统一进入所有已授权应用' },
    { icon: KeyRound, text: '权限边界清晰，访问范围始终可见' },
    { icon: ShieldCheck, text: '每一次授权与撤销都留有审计记录' },
]
</script>
<template>
    <main class="auth-layout">
        <aside class="auth-visual" aria-label="cobotusr 身份与访问管理">
            <div class="auth-brand">
                <span class="auth-brand-mark"><Boxes :size="18" aria-hidden="true" /></span>
                <span>cobot<span class="text-white/60">usr</span></span>
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
                企业统一身份认证 · cobotusr
            </p>
        </aside>
        <section class="auth-panel">
            <div class="auth-panel-top"><ThemeSwitcher /></div>
            <div class="auth-form">
                <div class="auth-mobile-brand">
                    <span class="brand-mark"><Boxes :size="17" aria-hidden="true" /></span>
                    <span>cobot<span class="text-primary">usr</span></span>
                </div>
                <div class="auth-eyebrow"><span class="status-dot" aria-hidden="true" />安全登录</div>
                <h1>{{ title }}</h1>
                <p>{{ description }}</p>
                <slot />
            </div>
            <p class="auth-legal">cobotusr · 身份与访问管理 · 受企业安全策略保护</p>
        </section>
    </main>
</template>
