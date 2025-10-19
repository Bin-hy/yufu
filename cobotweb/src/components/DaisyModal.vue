<template>
  <Teleport to="body">
    <transition name="fade">
      <div v-if="visible" class="modal modal-open" @click.self="onCancel">
        <div class="modal-box">
          <h3 class="font-bold text-lg">{{ title }}</h3>
          <div class="py-3">{{ content }}</div>
          <div class="modal-action">
            <button class="btn btn-primary" @click="onConfirm">{{ confirmText }}</button>
            <button class="btn" @click="onCancel">{{ cancelText }}</button>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<script setup lang="ts">
// 这是一个弹窗Modal组件，配合 '@/utils/modal' 的showModal 使用
import { ref, defineExpose } from 'vue'

const visible = ref(false)

const props = defineProps<{
  title: string
  content: string
  confirmText?: string
  cancelText?: string
}>()

const emit = defineEmits(['confirm', 'cancel', 'close'])

function open() {
  visible.value = true
}

function close() {
  visible.value = false
  emit('close')
}

function onConfirm() {
  emit('confirm')
  close()
}

function onCancel() {
  emit('cancel')
  close()
}

defineExpose({ open, close })
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
