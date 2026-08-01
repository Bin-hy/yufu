<script setup lang="ts">
import { RouterLink, RouterView } from 'vue-router'
import { LayoutGrid, Wrench, Radio, ScrollText, Plug, KeyRound } from '@lucide/vue'

const tabs = [
  { to: '/mcp/overview', label: '概览', icon: LayoutGrid },
  { to: '/mcp/tools', label: '工具', icon: Wrench },
  { to: '/mcp/sessions', label: '会话', icon: Radio },
  { to: '/mcp/audit', label: '审计', icon: ScrollText },
  { to: '/mcp/tokens', label: '访问令牌', icon: KeyRound },
  { to: '/mcp/connect', label: '接入与访问', icon: Plug },
]
</script>

<template>
  <div class="space-y-5">
    <div>
      <h2 class="text-lg font-semibold">MCP 服务</h2>
      <p class="text-xs text-muted-foreground mt-0.5">
        将在线设备能力以 Model Context Protocol 暴露给 AI 客户端
      </p>
    </div>

    <!-- tab bar -->
    <nav class="flex items-center gap-1 border-b border-border">
      <RouterLink
        v-for="t in tabs"
        :key="t.to"
        :to="t.to"
        class="inline-flex items-center gap-1.5 px-3 py-2 text-sm text-muted-foreground border-b-2 border-transparent hover:text-foreground transition-colors -mb-px"
        active-class="text-foreground border-primary font-medium"
      >
        <component :is="t.icon" class="w-3.5 h-3.5" />
        {{ t.label }}
      </RouterLink>
    </nav>

    <RouterView />
  </div>
</template>
