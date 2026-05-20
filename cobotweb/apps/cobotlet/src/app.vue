<script setup lang="ts">
import { RouterLink, RouterView } from 'vue-router'
</script>

<template>
  <div class="min-h-screen flex flex-col">
    <header class="border-b border-border px-6 py-3 flex items-center justify-between">
      <h1 class="text-base font-semibold">cobotlet 控制台</h1>
      <nav class="flex gap-4 text-sm">
        <RouterLink
          to="/devices"
          class="hover:underline"
          active-class="font-medium text-primary"
        >设备</RouterLink>
        <RouterLink
          to="/dispatch"
          class="hover:underline"
          active-class="font-medium text-primary"
        >任务派发</RouterLink>
      </nav>
    </header>
    <main class="flex-1 p-6">
      <RouterView />
    </main>
  </div>
</template>
