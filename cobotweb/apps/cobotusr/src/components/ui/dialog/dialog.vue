<script setup lang="ts">
import { ref, watchEffect, computed } from 'vue'
const props = withDefaults(
  defineProps<{
    open?: boolean
    title?: string
    size?: 'md' | 'lg' | 'xl'
    contentScrollable?: boolean
    hideFooter?: boolean
    confirmText?: string
    cancelText?: string
    closeOnConfirm?: boolean
  }>(),
  {
    open: false,
    size: 'md',
    contentScrollable: true,
    hideFooter: false,
    confirmText: '确认',
    cancelText: '取消',
    closeOnConfirm: true,
  }
)
const emit = defineEmits<{ (e: 'update:open', v: boolean): void; (e: 'confirm'): void; (e: 'cancel'): void }>()
const visible = ref(props.open)
watchEffect(() => { visible.value = !!props.open })
function close() { emit('update:open', false) }
function onConfirm() { emit('confirm'); if (props.closeOnConfirm) close() }
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
    <transition
      enter-active-class="transition-all duration-200 ease-out"
      leave-active-class="transition-all duration-150 ease-in"
      enter-from-class="opacity-0"
      leave-to-class="opacity-0"
    >
      <div v-if="visible" class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <!-- 背景遮罩 -->
        <div class="absolute inset-0 bg-black/50 backdrop-blur-sm" @click="onCancel" />

        <!-- 弹窗主体 -->
        <transition
          enter-active-class="transition-all duration-200 ease-out"
          leave-active-class="transition-all duration-150 ease-in"
          enter-from-class="opacity-0 scale-95 translate-y-2"
          leave-to-class="opacity-0 scale-95 translate-y-2"
        >
          <div
            v-if="visible"
            class="relative z-10 w-full rounded-2xl border border-border bg-card text-card-foreground shadow-2xl"
            :class="sizeClass"
          >
            <!-- 标题栏 -->
            <div class="flex items-center justify-between px-6 pt-6 pb-4">
              <h3 class="text-base font-semibold text-foreground">{{ props.title }}</h3>
              <button
                class="rounded-lg p-1.5 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                @click="onCancel"
              >
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>

            <!-- 内容区 -->
            <div class="px-6 pb-6" :class="props.contentScrollable ? 'max-h-[60vh] overflow-y-auto' : ''">
              <slot />
            </div>

            <!-- 底部按钮 -->
            <div v-if="!props.hideFooter" class="flex items-center justify-end gap-2 px-6 py-4 border-t border-border bg-muted/40 rounded-b-2xl">
              <button
                class="inline-flex h-9 items-center justify-center rounded-xl border border-input bg-background px-4 text-sm font-medium text-foreground hover:bg-accent transition-colors"
                @click="onCancel"
              >
                {{ props.cancelText }}
              </button>
              <button
                class="inline-flex h-9 items-center justify-center rounded-xl bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
                @click="onConfirm"
              >
                {{ props.confirmText }}
              </button>
            </div>
          </div>
        </transition>
      </div>
    </transition>
  </Teleport>
</template>
