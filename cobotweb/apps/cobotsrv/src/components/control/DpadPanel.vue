<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import type { ControlPreset } from '@cobotweb/shared'

const props = defineProps<{
  preset: ControlPreset
  activeKeys: Set<string>
  press: (key: string) => void
  release: (key: string) => void
}>()

const DPAD_KEYS = ['forward', 'backward', 'left', 'right', 'stop']
const KEY_MAP: Record<string, string> = {
  w: 'forward', arrowup: 'forward',
  s: 'backward', arrowdown: 'backward',
  a: 'left', arrowleft: 'left',
  d: 'right', arrowright: 'right',
  ' ': 'stop',
}

const available = DPAD_KEYS.filter((k) => k in props.preset.mapping)

function onKeyDown(e: KeyboardEvent) {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return
  const mapped = KEY_MAP[e.key.toLowerCase()]
  if (mapped && props.preset.mapping[mapped]) {
    e.preventDefault()
    props.press(mapped)
  }
}

function onKeyUp(e: KeyboardEvent) {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return
  const mapped = KEY_MAP[e.key.toLowerCase()]
  if (mapped && props.preset.mapping[mapped]) {
    e.preventDefault()
    props.release(mapped)
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown)
  window.addEventListener('keyup', onKeyUp)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown)
  window.removeEventListener('keyup', onKeyUp)
})
</script>

<template>
  <div class="flex flex-col items-center gap-2 select-none">
    <p class="text-xs text-muted-foreground mb-1">键盘 WASD / 方向键可控</p>
    <div class="grid grid-cols-3 grid-rows-3 gap-1.5 w-44 h-44">
      <!-- row 1: top center = forward -->
      <div></div>
      <button
        v-if="available.includes('forward')"
        class="dpad-btn"
        :class="{ active: activeKeys.has('forward') }"
        @pointerdown.prevent="press('forward')"
        @pointerup.prevent="release('forward')"
        @pointerleave="release('forward')"
      >▲</button>
      <div v-else></div>
      <div></div>
      <!-- row 2: left, stop, right -->
      <button
        v-if="available.includes('left')"
        class="dpad-btn"
        :class="{ active: activeKeys.has('left') }"
        @pointerdown.prevent="press('left')"
        @pointerup.prevent="release('left')"
        @pointerleave="release('left')"
      >◄</button>
      <button
        v-if="available.includes('stop')"
        class="dpad-btn stop"
        :class="{ active: activeKeys.has('stop') }"
        @pointerdown.prevent="press('stop')"
        @pointerup.prevent="release('stop')"
        @pointerleave="release('stop')"
      >■</button>
      <div v-else></div>
      <button
        v-if="available.includes('right')"
        class="dpad-btn"
        :class="{ active: activeKeys.has('right') }"
        @pointerdown.prevent="press('right')"
        @pointerup.prevent="release('right')"
        @pointerleave="release('right')"
      >►</button>
      <!-- row 3: bottom center = backward -->
      <div></div>
      <button
        v-if="available.includes('backward')"
        class="dpad-btn"
        :class="{ active: activeKeys.has('backward') }"
        @pointerdown.prevent="press('backward')"
        @pointerup.prevent="release('backward')"
        @pointerleave="release('backward')"
      >▼</button>
      <div v-else></div>
      <div></div>
    </div>
  </div>
</template>

<style scoped>
.dpad-btn {
  @apply flex items-center justify-center rounded-lg border border-border bg-card text-lg font-bold
         hover:bg-muted active:bg-primary/20 transition-colors cursor-pointer select-none touch-none;
}
.dpad-btn.active {
  @apply bg-primary/30 border-primary text-primary;
}
.dpad-btn.stop {
  @apply text-destructive;
}
</style>
