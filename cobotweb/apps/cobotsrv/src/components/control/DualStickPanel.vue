<script setup lang="ts">
import { ref, onUnmounted } from 'vue'
import type { ControlPreset } from '@cobotweb/shared'

const props = defineProps<{
  preset: ControlPreset
  activeKeys: Set<string>
  press: (key: string) => void
  release: (key: string) => void
}>()

const STICK_KEYS = {
  left: ['stick_left_x', 'stick_left_y'],
  right: ['stick_right_x', 'stick_right_y'],
}

const leftPos = ref({ x: 0, y: 0 })
const rightPos = ref({ x: 0, y: 0 })

const stickRadius = 50
let activeDrag: 'left' | 'right' | null = null
let sendTimer: ReturnType<typeof setInterval> | null = null

function startDrag(stick: 'left' | 'right', e: PointerEvent) {
  activeDrag = stick
  const el = e.currentTarget as HTMLElement
  el.setPointerCapture(e.pointerId)
  startSending(stick)
}

function onMove(stick: 'left' | 'right', e: PointerEvent) {
  if (activeDrag !== stick) return
  const el = e.currentTarget as HTMLElement
  const rect = el.getBoundingClientRect()
  const cx = rect.width / 2
  const cy = rect.height / 2
  let dx = e.clientX - rect.left - cx
  let dy = e.clientY - rect.top - cy
  const dist = Math.sqrt(dx * dx + dy * dy)
  if (dist > stickRadius) {
    dx = (dx / dist) * stickRadius
    dy = (dy / dist) * stickRadius
  }
  const pos = { x: +(dx / stickRadius).toFixed(2), y: +(-dy / stickRadius).toFixed(2) }
  if (stick === 'left') leftPos.value = pos
  else rightPos.value = pos
}

function endDrag(stick: 'left' | 'right') {
  if (activeDrag !== stick) return
  activeDrag = null
  if (stick === 'left') leftPos.value = { x: 0, y: 0 }
  else rightPos.value = { x: 0, y: 0 }
  stopSending()
  const keys = STICK_KEYS[stick]
  keys.forEach((k) => props.release(k))
}

function startSending(stick: 'left' | 'right') {
  const keys = STICK_KEYS[stick]
  const interval = props.preset.mapping[keys[0]]?.interval_ms ?? 200
  keys.forEach((k) => props.press(k))
  sendTimer = setInterval(() => {
    keys.forEach((k) => props.press(k))
  }, interval)
}

function stopSending() {
  if (sendTimer) {
    clearInterval(sendTimer)
    sendTimer = null
  }
}

onUnmounted(stopSending)

function stickStyle(pos: { x: number; y: number }) {
  return { transform: `translate(${pos.x * stickRadius}px, ${-pos.y * stickRadius}px)` }
}
</script>

<template>
  <div class="flex items-center justify-center gap-8 select-none">
    <div v-for="stick in (['left', 'right'] as const)" :key="stick" class="flex flex-col items-center gap-1">
      <span class="text-xs text-muted-foreground">{{ stick === 'left' ? '移动' : '视角' }}</span>
      <div
        class="relative w-32 h-32 rounded-full border-2 border-border bg-muted/30 touch-none cursor-pointer"
        @pointerdown.prevent="startDrag(stick, $event)"
        @pointermove="onMove(stick, $event)"
        @pointerup="endDrag(stick)"
        @pointercancel="endDrag(stick)"
      >
        <div class="absolute inset-0 flex items-center justify-center">
          <div
            class="w-10 h-10 rounded-full bg-primary/60 border border-primary shadow transition-transform"
            :style="stickStyle(stick === 'left' ? leftPos : rightPos)"
          ></div>
        </div>
      </div>
      <span class="text-[10px] text-muted-foreground font-mono">
        x: {{ (stick === 'left' ? leftPos : rightPos).x }}
        y: {{ (stick === 'left' ? leftPos : rightPos).y }}
      </span>
    </div>
  </div>
</template>
