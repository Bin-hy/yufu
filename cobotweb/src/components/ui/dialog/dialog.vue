<script setup lang="ts">
import { ref, watchEffect } from 'vue'
const props = withDefaults(defineProps<{ open?: boolean; title?: string }>(), { open: false })
const emit = defineEmits<{ (e: 'update:open', v: boolean): void; (e: 'confirm'): void; (e: 'cancel'): void }>()
const visible = ref(props.open)
watchEffect(() => { visible.value = !!props.open })
function close() { emit('update:open', false) }
function onConfirm() { emit('confirm'); close() }
function onCancel() { emit('cancel'); close() }
</script>

<template>
  <Teleport to="body">
    <transition name="fade">
      <div v-if="visible" class="fixed inset-0 z-50 flex items-center justify-center">
        <div class="absolute inset-0 bg-black/40" @click="onCancel" />
        <div class="relative z-10 w-full max-w-md rounded-lg border border-border bg-card text-card-foreground shadow-lg">
          <div class="px-4 py-3 border-b border-border">
            <h3 class="text-base font-semibold">{{ props.title }}</h3>
          </div>
          <div class="px-4 py-3">
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