<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { ArrowRight, CalendarDays, Command, KeyRound, ShieldCheck, Sparkles, UserRound } from '@lucide/vue'
import { navigationFor } from '@/router/navigation'
import PageHeader from '@/components/workspace/page-header.vue'
import UiButton from '@/components/ui/button/button.vue'
import { useUserStore } from '@/pinia/stores/user'
import { useCommandStore } from '@/pinia/stores/command'
import { useShortcutLabel } from '@/composables/use-platform'
const user = useUserStore(), router = useRouter(), command = useCommandStore(),
    { commandKey } = useShortcutLabel()
const descriptions: Record<string, string> = {
    'account-profile': '更新显示名称、邮箱与账户安全设置。',
    'account-apps': '查看企业已明确分配给你的应用环境。',
    'account-consents': '检查并撤回应用对账户资料的访问授权。',
    'account-sessions': '查看登录设备与会话，及时撤销异常访问。',
    'developer-apps': '注册与维护应用，管理开发和生产的接入配置。',
    'admin-accounts': '邀请成员、调整账户资格并签发重置链接。',
    'admin-groups': '用用户组汇总成员与已分配的应用访问。',
    'admin-apps': '审批生产接入，查看应用配置与访问分配。',
    'admin-migrations': '扫描历史客户端，明确归属后纳入统一治理。',
    'admin-audit': '查询账户、应用与授权操作的完整摘要。',
    'admin-deliveries': '追踪退出通知投递与失败重试状态。',
}
const shortcuts = computed(() =>
    navigationFor(user.centerSession)
        .flatMap((group) => group.items)
        .filter((item) => item.name !== 'dashboard-home')
        .map((item) => ({
            ...item,
            group: navigationFor(user.centerSession).find((group) => group.items.includes(item))?.title ?? '',
            description: descriptions[item.name] ?? '进入' + item.title + '。',
        })),
)
const displayName = computed(
    () => user.userInfo?.nickname || user.userInfo?.username || '欢迎回来',
)
const avatar = computed(() => displayName.value.slice(0, 1).toUpperCase())
const roleLabel = computed(() =>
    user.centerSession?.administrator
        ? '企业管理员'
        : user.centerSession?.developer_enabled
            ? '开发者'
            : '企业成员',
)
const today = new Intl.DateTimeFormat('zh-CN', {
    month: 'long',
    day: 'numeric',
    weekday: 'long',
}).format(new Date())
const expiry = computed(() => {
    const value = user.centerSession?.expires_at
    return value ? new Date(value * 1000).toLocaleString('zh-CN') : '当前会话'
})
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="工作台"
            eyebrow="OVERVIEW"
            description="从这里管理身份、访问权限和应用授权。"
        >
            <UiButton variant="outline" @click="command.show()">
                <Command :size="14" aria-hidden="true" />搜索页面
                <kbd class="kbd">{{ commandKey }}</kbd>
            </UiButton>
        </PageHeader>

        <section class="hero-panel">
            <div class="hero-copy">
                <div class="page-eyebrow !mb-4">IDENTITY & ACCESS</div>
                <h2>你好，{{ displayName }}</h2>
                <p>
                    当前账户已通过企业统一身份认证。你的访问范围、应用授权与会话状态，都可以在这里集中查看。
                </p>
                <div class="flex flex-wrap items-center gap-2.5">
                    <UiButton @click="router.push('/account')">
                        账户中心<ArrowRight :size="14" aria-hidden="true" />
                    </UiButton>
                    <UiButton variant="outline" @click="router.push('/account/sessions')">
                        查看登录会话
                    </UiButton>
                </div>
                <div class="hero-facts">
                    <div><span>统一身份</span><strong>已验证</strong></div>
                    <div><span>会话保护</span><strong>已启用</strong></div>
                    <div><span>审计记录</span><strong>持续写入</strong></div>
                </div>
                <p v-if="user.identityError" role="alert" class="inline-error mt-4">
                    {{ user.identityError }}
                    <button type="button" class="underline" @click="user.ensureSession">重新获取身份</button>
                </p>
            </div>
            <aside class="identity-card" aria-label="当前身份">
                <div class="identity-card-top">
                    <span class="identity-avatar">{{ avatar }}</span>
                    <span class="status-badge status-success">
                        <span class="status-dot" aria-hidden="true" />已验证
                    </span>
                </div>
                <div>
                    <p class="identity-name">{{ displayName }}</p>
                    <p class="identity-username">@{{ user.centerSession?.username }}</p>
                </div>
                <dl class="identity-list">
                    <div><dt>账户角色</dt><dd>{{ roleLabel }}</dd></div>
                    <div><dt>人员类型</dt><dd>{{ user.centerSession?.person_kind === 'employee' ? '企业员工' : '受邀合作方' }}</dd></div>
                    <div><dt>会话有效至</dt><dd>{{ expiry }}</dd></div>
                </dl>
                <div class="identity-card-footer">
                    <ShieldCheck :size="14" aria-hidden="true" />
                    主体 ID {{ user.centerSession?.sub }}
                </div>
            </aside>
        </section>

        <div class="metric-grid mt-4">
            <article class="metric-card">
                <span class="metric-label"><CalendarDays :size="13" aria-hidden="true" />今天</span>
                <strong class="metric-value !text-base">{{ today }}</strong>
                <span class="metric-meta">身份服务运行正常</span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><UserRound :size="13" aria-hidden="true" />账户资格</span>
                <strong class="metric-value !text-base">{{ roleLabel }}</strong>
                <span class="metric-meta">
                    {{ user.centerSession?.developer_enabled ? '具备开发者控制台入口' : '由企业管理员维护访问范围' }}
                </span>
            </article>
            <article class="metric-card">
                <span class="metric-label"><KeyRound :size="13" aria-hidden="true" />工作入口</span>
                <strong class="metric-value">{{ shortcuts.length }}</strong>
                <span class="metric-meta">个与你当前资格匹配的控制台入口</span>
            </article>
        </div>

        <div class="page-section-title">
            <div>
                <h2>常用工作区</h2>
                <p class="mt-0.5">按任务选择入口，页面状态会自动保留。</p>
            </div>
            <span class="hidden text-[11px] text-muted-foreground sm:block">共 {{ shortcuts.length }} 个入口</span>
        </div>
        <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            <router-link
                v-for="item in shortcuts"
                :key="item.name"
                :to="{ name: item.name }"
                class="shortcut-card group"
            >
                <span class="shortcut-icon">
                    <component :is="item.icon" :size="18" aria-hidden="true" />
                </span>
                <span class="min-w-0 flex-1">
                    <span class="shortcut-group">{{ item.group }}</span>
                    <span class="shortcut-title">{{ item.title }}</span>
                    <span class="shortcut-description">{{ item.description }}</span>
                </span>
                <ArrowRight
                    :size="15"
                    class="mt-0.5 shrink-0 text-muted-foreground/50 transition-all group-hover:translate-x-0.5 group-hover:text-primary"
                    aria-hidden="true"
                />
            </router-link>
        </div>

        <div class="mt-5 inline-notice">
            <Sparkles :size="15" class="mt-0.5 shrink-0 text-primary" aria-hidden="true" />
            <p>
                <strong>小提示：</strong>按 <kbd class="kbd">{{ commandKey }}</kbd> 可以随时搜索页面、切换主题或退出登录。
            </p>
        </div>
    </div>
</template>

<style scoped>
.hero-panel {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(280px, 0.65fr);
    gap: 28px;
    padding: 28px;
    border: 1px solid hsl(var(--border));
    border-radius: 18px;
    background:
        radial-gradient(700px 320px at 100% 0%, hsl(var(--primary) / 0.12), transparent 62%),
        hsl(var(--card));
    box-shadow: var(--shadow-sm);
}
.hero-copy h2 {
    margin: 0;
    font-size: clamp(26px, 3vw, 34px);
    line-height: 1.16;
    font-weight: 650;
    letter-spacing: -0.045em;
}
.hero-copy > p {
    max-width: 560px;
    margin: 10px 0 20px;
    color: hsl(var(--muted-foreground));
    font-size: 12.5px;
    line-height: 1.75;
}
.hero-facts {
    display: flex;
    flex-wrap: wrap;
    gap: 22px;
    margin-top: 26px;
    padding-top: 18px;
    border-top: 1px solid hsl(var(--border));
}
.hero-facts > div {
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.hero-facts span {
    color: hsl(var(--muted-foreground));
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
}
.hero-facts strong {
    font-size: 11.5px;
    font-weight: 650;
}
.identity-card {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
    border: 1px solid hsl(var(--border));
    border-radius: 14px;
    background: hsl(var(--card) / 0.75);
    backdrop-filter: blur(10px);
    box-shadow: var(--shadow-sm);
}
.identity-card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
}
.identity-avatar {
    width: 46px;
    height: 46px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 13px;
    background: linear-gradient(145deg, hsl(var(--primary)), hsl(var(--primary) / 0.68));
    color: hsl(var(--primary-foreground));
    font-size: 18px;
    font-weight: 700;
    box-shadow: 0 8px 20px -8px hsl(var(--primary) / 0.7);
}
.identity-name {
    margin: 0;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.035em;
}
.identity-username {
    margin-top: 2px;
    color: hsl(var(--muted-foreground));
    font-size: 11.5px;
}
.identity-list {
    display: grid;
    gap: 8px;
    margin: 0;
    padding-top: 14px;
    border-top: 1px solid hsl(var(--border));
}
.identity-list > div {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
}
.identity-list dt {
    color: hsl(var(--muted-foreground));
    font-size: 10.5px;
}
.identity-list dd {
    margin: 0;
    font-size: 11.5px;
    font-weight: 600;
    text-align: right;
}
.identity-card-footer {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: auto;
    padding-top: 12px;
    border-top: 1px solid hsl(var(--border));
    color: hsl(var(--muted-foreground));
    font-size: 10.5px;
}
.shortcut-card {
    display: flex;
    align-items: flex-start;
    gap: 13px;
    padding: 16px;
    border: 1px solid hsl(var(--border));
    border-radius: 12px;
    background: hsl(var(--card));
    box-shadow: var(--shadow-sm);
    transition:
        border-color 140ms ease,
        box-shadow 140ms ease,
        transform 140ms ease;
}
.shortcut-card:hover {
    border-color: hsl(var(--primary) / 0.4);
    box-shadow: var(--shadow-md);
    transform: translateY(-1px);
}
.shortcut-icon {
    width: 38px;
    height: 38px;
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 10px;
    background: hsl(var(--secondary));
    color: hsl(var(--secondary-foreground));
}
.shortcut-group {
    display: block;
    color: hsl(var(--muted-foreground));
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    margin-bottom: 3px;
}
.shortcut-title {
    display: block;
    font-size: 13.5px;
    font-weight: 650;
    letter-spacing: -0.01em;
}
.shortcut-description {
    display: block;
    margin-top: 4px;
    color: hsl(var(--muted-foreground));
    font-size: 11.5px;
    line-height: 1.55;
}
@media (max-width: 1023px) {
    .hero-panel {
        grid-template-columns: minmax(0, 1fr);
    }
}
</style>
