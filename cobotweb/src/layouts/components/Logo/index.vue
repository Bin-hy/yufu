<template>
  <div class="flex items-center px-4 h-14 gap-2">
    <img src="/vite.svg" alt="logo" class="w-6 h-6" />
    <span class="font-bold">Cobotrix Admin</span>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{ collapse?: boolean }>()
</script>
