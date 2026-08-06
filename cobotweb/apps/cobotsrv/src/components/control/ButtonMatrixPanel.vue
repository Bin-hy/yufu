<script setup lang="ts">
import type { ControlPreset } from '@cobotweb/shared'

const props = defineProps<{
  preset: ControlPreset
  activeKeys: Set<string>
  press: (key: string) => void
  release: (key: string) => void
}>()

const buttons = Object.entries(props.preset.mapping)
</script>

<template>
  <div class="grid grid-cols-3 sm:grid-cols-4 gap-2 select-none">
    <button
      v-for="[key, binding] in buttons"
      :key="key"
      class="relative rounded-lg border px-3 py-4 text-sm font-medium transition-colors touch-none"
      :class="[
        activeKeys.has(key)
          ? 'bg-primary/30 border-primary text-primary'
          : binding.mode === 'fire_once'
            ? 'border-blue-400/50 bg-blue-500/5 hover:bg-blue-500/10 text-blue-600'
            : 'border-orange-400/50 bg-orange-500/5 hover:bg-orange-500/10 text-orange-600',
      ]"
      @pointerdown.prevent="press(key)"
      @pointerup.prevent="release(key)"
      @pointerleave="release(key)"
    >
      <span class="block truncate">{{ key }}</span>
      <span class="absolute bottom-1 right-1.5 text-[9px] opacity-60">
        {{ binding.mode === 'fire_once' ? '单击' : '按住' }}
      </span>
    </button>
  </div>
</template>
