<script setup>
import { reactive, computed } from 'vue'
import UiButton from '@/components/ui/button/button.vue'

</script>

<template>
  <div class="max-w-xl mx-auto p-6">
    <h1 class="text-xl font-semibold mb-4">个人中心</h1>
    <div class="rounded-lg bg-card text-card-foreground shadow">
      <div class="space-y-4 p-6">
        <div class="flex items-center gap-3">
          <div class="rounded-full w-12 h-12 bg-primary text-primary-foreground flex items-center justify-center">
            <span class="text-lg">{{ (state.nickname || state.username || '我')[0] }}</span>
          </div>
          <div>
            <div class="text-sm text-muted-foreground">用户名</div>
            <div class="font-medium">{{ state.username }}</div>
          </div>
        </div>

      </div>
    </div>
  </div>
</template>
