<script setup lang="ts">
import {
    ArrowUpRight,
    Users,
    ShieldCheck,
    AppWindow,
    KeyRound,
    ScrollText,
    Network,
} from '@lucide/vue'
import PageHeader from '@/components/workspace/page-header.vue'
import { useUserStore } from '@/pinia/stores/user'
const user = useUserStore()
const shortcuts = [
    {
        name: 'user-list',
        title: '用户与身份',
        description: '管理用户账号与角色，维护身份访问边界。',
        icon: Users,
    },
    {
        name: 'authority-assign',
        title: 'API 访问控制',
        description: '为角色分配接口访问权限。',
        icon: Network,
    },
    {
        name: 'permission-assign',
        title: '资源权限',
        description: '配置资源范围与操作能力。',
        icon: ShieldCheck,
    },
    {
        name: 'application-list',
        title: '应用注册',
        description: '管理应用、OAuth 客户端与密钥。',
        icon: AppWindow,
    },
    {
        name: 'authorized-apps-list',
        title: '授权管理',
        description: '查看已授权应用，管理或撤销访问。',
        icon: KeyRound,
    },
    {
        name: 'recordings-list',
        title: '操作审计',
        description: '检查操作记录，追踪用户活动。',
        icon: ScrollText,
    },
]
</script>
<template>
    <div class="workspace-page">
        <PageHeader
            title="工作台"
            eyebrow="OVERVIEW"
            description="从这里管理身份、访问权限和应用授权。"
        />
        <section
            class="relative mb-6 overflow-hidden rounded-xl border border-primary/15 bg-card p-6 sm:p-8"
        >
            <div
                class="absolute right-0 top-0 h-48 w-48 rounded-full bg-primary/5 blur-3xl"
                aria-hidden="true"
            />
            <div class="relative">
                <p class="mb-3 text-xs font-medium text-primary">IDENTITY & ACCESS</p>
                <h2 class="text-xl font-semibold">
                    你好，{{ user.userInfo?.nickname || user.userInfo?.username || '欢迎回来' }}
                </h2>
                <p class="mt-2 max-w-lg text-sm leading-6 text-muted-foreground">
                    保持工作上下文，专注当前任务。打开的页面会保留分页与筛选，未保存的修改会在离开前提醒。
                </p>
                <router-link to="/users/create" class="ui-button ui-button-default mt-5"
                    >创建用户<ArrowUpRight :size="15" aria-hidden="true"
                /></router-link>
                <p v-if="user.identityError" role="alert" class="inline-error mt-4">
                    {{ user.identityError }}
                    <button type="button" class="underline" @click="user.ensureSession">
                        重新获取身份
                    </button>
                </p>
            </div>
        </section>
        <div class="mb-4 flex items-center justify-between">
            <h2 class="font-semibold">常用工作区</h2>
            <span class="text-xs text-muted-foreground">按任务选择入口</span>
        </div>
        <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <router-link
                v-for="item in shortcuts"
                :key="item.name"
                :to="{ name: item.name }"
                class="group rounded-lg border border-border bg-card p-5 transition-colors hover:border-primary/40"
                ><div class="mb-5 flex items-center justify-between">
                    <span
                        class="flex h-9 w-9 items-center justify-center rounded-lg bg-secondary text-secondary-foreground"
                        ><component :is="item.icon" :size="18" aria-hidden="true" /></span
                    ><ArrowUpRight
                        :size="16"
                        class="text-muted-foreground group-hover:text-primary"
                        aria-hidden="true"
                    />
                </div>
                <h3 class="text-sm font-semibold">{{ item.title }}</h3>
                <p class="mt-2 text-xs leading-5 text-muted-foreground">
                    {{ item.description }}
                </p></router-link
            >
        </div>
        <div
            class="mt-6 flex flex-wrap items-start justify-between gap-5 rounded-lg border border-border bg-muted/40 p-5"
        >
            <div>
                <h2 class="text-sm font-medium">开始配置访问权限</h2>
                <p class="mt-1 text-xs leading-5 text-muted-foreground">
                    先建立角色，再分配 API 与资源权限。应用授权使用既有 OAuth / OIDC 流程。
                </p>
            </div>
            <router-link to="/roles" class="ui-button ui-button-outline"
                >管理角色<ArrowUpRight :size="14" aria-hidden="true"
            /></router-link>
        </div>
    </div>
</template>
