<script setup lang="ts">
import { ref } from 'vue'
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
// Track both sticks independently so the two can be dragged at the same time.
const leftActive = ref(false)
const rightActive = ref(false)

function isActive(stick: 'left' | 'right'): boolean {
  return stick === 'left' ? leftActive.value : rightActive.value
}

function setActive(stick: 'left' | 'right', active: boolean) {
  if (stick === 'left') leftActive.value = active
  else rightActive.value = active
}

function startDrag(stick: 'left' | 'right', e: PointerEvent) {
  setActive(stick, true)
  const el = e.currentTarget as HTMLElement
  el.setPointerCapture(e.pointerId)
  // press() starts the per-key continuous timer when the binding is
  // `continuous`; press_release / fire_once send one command immediately.
  STICK_KEYS[stick].forEach((k) => props.press(k))
}

function onMove(stick: 'left' | 'right', e: PointerEvent) {
  if (!isActive(stick)) return
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
  if (!isActive(stick)) return
  setActive(stick, false)
  if (stick === 'left') leftPos.value = { x: 0, y: 0 }
  else rightPos.value = { x: 0, y: 0 }
  // release() stops the continuous timer and sends release_params for
  // press_release bindings.
  STICK_KEYS[stick].forEach((k) => props.release(k))
}

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
