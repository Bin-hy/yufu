<script setup lang="ts">
import { ref, watchEffect, computed } from 'vue'
const props = withDefaults(
  defineProps<{ open?: boolean; title?: string; size?: 'md' | 'lg' | 'xl'; contentScrollable?: boolean }>(),
  { open: false, size: 'md', contentScrollable: true }
)
const emit = defineEmits<{ (e: 'update:open', v: boolean): void; (e: 'confirm'): void; (e: 'cancel'): void }>()
const visible = ref(props.open)
watchEffect(() => { visible.value = !!props.open })
function close() { emit('update:open', false) }
function onConfirm() { emit('confirm'); close() }
function onCancel() { emit('cancel'); close() }

const sizeClass = computed(() => {
  switch (props.size) {
    case 'xl': return 'max-w-4xl'
    case 'lg': return 'max-w-2xl'
    default: return 'max-w-md'
  }
})
</script>

<template>
  <Teleport to="body">
    <transition name="fade">
      <div v-if="visible" class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div class="absolute inset-0 bg-black/40" @click="onCancel" />
        <div class="relative z-10 w-full" :class="['rounded-lg border border-border bg-card text-card-foreground shadow-lg', sizeClass]">
          <div class="px-4 py-3 border-b border-border">
            <h3 class="text-base font-semibold">{{ props.title }}</h3>
          </div>
          <div class="px-4 py-3" :class="props.contentScrollable ? 'max-h-[70vh] overflow-y-auto' : ''">
            <slot />
          </div>
          <div class="px-4 py-3 border-t border-border flex justify-end gap-2">
            <button class="inline-flex h-9 items-center justify-center rounded-md border border-input bg-background px-4 text-sm hover:bg-accent hover:text-accent-foreground" @click="onCancel">取消</button>
            <button class="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm text-primary-foreground hover:bg-primary/90" @click="onConfirm">确认</button>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active { transition: opacity 0.2s; }
.fade-enter-from,
.fade-leave-to { opacity: 0; }
</style>