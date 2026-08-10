<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useMediaRecorder } from "@/composables/useMediaRecorder";

const emit = defineEmits<{
  recorded: [file: File, durationSec: number];
  error: [message: string];
}>();

const recorder = useMediaRecorder("audio");

// Track pointer position for slide-to-cancel
const pointerY = ref(0);
const startY = ref(0);
const cancelThreshold = 80;
const isCancelZone = ref(false);

// Recording overlay visibility
const showOverlay = ref(false);

const showCancel = computed(() => isCancelZone.value);
const state = computed(() => recorder.state.value);
const durationSec = computed(() => recorder.durationSec.value);
const durationDisplay = computed(() => {
  const totalSec = Math.floor(durationSec.value);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${String(min).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
});

const isDisabled = computed(() => state.value === "requesting" || state.value === "recording");

async function startRecording(e: PointerEvent) {
  if (state.value === "recording" || state.value === "requesting") return;
  startY.value = e.clientY;
  pointerY.value = e.clientY;
  isCancelZone.value = false;
  showOverlay.value = true;

  await recorder.start();

  if (state.value === "denied" && recorder.error.value) {
    emit("error", recorder.error.value);
    showOverlay.value = false;
  }

  // Add global listeners for release outside button
  window.addEventListener("pointerup", onPointerUp);
  window.addEventListener("pointermove", onPointerMove);
}

function onPointerMove(e: PointerEvent) {
  pointerY.value = e.clientY;
  const deltaY = startY.value - pointerY.value;
  isCancelZone.value = deltaY > cancelThreshold;
}

function onPointerUp(_e: PointerEvent) {
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointermove", onPointerMove);

  if (state.value !== "recording") {
    showOverlay.value = false;
    return;
  }

  if (isCancelZone.value) {
    recorder.cancel();
    showOverlay.value = false;
    isCancelZone.value = false;
    return;
  }

  recorder.stop().then((file) => {
    showOverlay.value = false;
    isCancelZone.value = false;
    if (file) {
      const dur = Math.round(recorder.durationSec.value);
      emit("recorded", file, dur);
    } else {
      // too short
      emit("error", "录音时间太短");
    }
  });
}

// Touch events (prevent scrolling during recording)
function onTouchStart(e: TouchEvent) {
  e.preventDefault();
  if (state.value === "recording" || state.value === "requesting") return;
  startY.value = e.touches[0].clientY;
  pointerY.value = e.touches[0].clientY;
  isCancelZone.value = false;
  showOverlay.value = true;

  recorder.start().then(() => {
    if (state.value === "denied" && recorder.error.value) {
      emit("error", recorder.error.value);
      showOverlay.value = false;
    }
  });

  window.addEventListener("touchend", onTouchEnd, { passive: false });
  window.addEventListener("touchmove", onTouchMove, { passive: false });
}

function onTouchMove(e: TouchEvent) {
  e.preventDefault();
  pointerY.value = e.touches[0].clientY;
  const deltaY = startY.value - pointerY.value;
  isCancelZone.value = deltaY > cancelThreshold;
}

function onTouchEnd(e: TouchEvent) {
  e.preventDefault();
  window.removeEventListener("touchend", onTouchEnd);
  window.removeEventListener("touchmove", onTouchMove);

  if (state.value !== "recording") {
    showOverlay.value = false;
    return;
  }

  if (isCancelZone.value) {
    recorder.cancel();
    showOverlay.value = false;
    isCancelZone.value = false;
    return;
  }

  recorder.stop().then((file) => {
    showOverlay.value = false;
    isCancelZone.value = false;
    if (file) {
      const dur = Math.round(recorder.durationSec.value);
      emit("recorded", file, dur);
    } else {
      emit("error", "录音时间太短");
    }
  });
}

// Keyboard: hold Space to record
function onKeyDown(e: KeyboardEvent) {
  if (e.key === " " && !e.repeat && state.value === "idle") {
    e.preventDefault();
    showOverlay.value = true;
    recorder.start().then(() => {
      if (state.value === "denied" && recorder.error.value) {
        emit("error", recorder.error.value);
        showOverlay.value = false;
      }
    });
  }
}

function onKeyUp(e: KeyboardEvent) {
  if (e.key === " " && state.value === "recording") {
    e.preventDefault();
    recorder.stop().then((file) => {
      showOverlay.value = false;
      if (file) {
        const dur = Math.round(recorder.durationSec.value);
        emit("recorded", file, dur);
      } else {
        emit("error", "录音时间太短");
      }
    });
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("keyup", onKeyUp);
});
</script>

<template>
  <div class="flex flex-col items-center">
    <!-- Record button -->
    <button
      class="flex flex-col items-center justify-center px-3 py-1.5 rounded-lg transition-colors disabled:opacity-50"
      :class="showCancel ? 'bg-red-100 text-red-600' : 'hover:bg-gray-100 text-gray-600'"
      :disabled="isDisabled"
      @pointerdown.prevent="startRecording"
      @touchstart.prevent="onTouchStart"
      title="按住录音（或长按空格键）"
    >
      <span class="text-xl leading-none mb-0.5">{{ showCancel ? '✕' : '🎤' }}</span>
      <span class="text-xs leading-tight">按住录音</span>
    </button>

    <!-- Recording overlay -->
    <Teleport to="body">
      <div
        v-if="showOverlay && state === 'recording'"
        class="fixed inset-0 z-50 flex flex-col items-center justify-center transition-colors duration-200"
        :class="showCancel ? 'bg-red-900/80' : 'bg-black/70'"
        style="backdrop-filter: blur(4px)"
      >
        <div class="flex flex-col items-center gap-6">
          <!-- Timer -->
          <span class="text-4xl font-mono text-white tracking-wider">
            {{ durationDisplay }}
          </span>

          <!-- Waveform bars -->
          <div class="flex items-center gap-1.5 h-12">
            <span
              v-for="i in 5"
              :key="i"
              class="w-1.5 rounded-full transition-colors duration-200"
              :class="showCancel ? 'bg-red-300' : 'bg-red-400'"
              :style="{
                height: '12px',
                animation: `recording-wave 0.6s ease-in-out infinite`,
                animationDelay: `${i * 0.1}s`,
              }"
            />
          </div>

          <!-- Pulse / Cancel indicator -->
          <div
            class="w-16 h-16 rounded-full flex items-center justify-center transition-colors duration-200"
            :class="showCancel ? 'bg-red-500' : 'bg-red-500 animate-pulse'"
          >
            <span v-if="showCancel" class="text-white text-2xl font-bold">✕</span>
          </div>

          <!-- Hint text -->
          <p class="text-sm text-gray-300 select-none">
            {{ showCancel ? '松开取消' : '松开发送，上滑取消' }}
          </p>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
@keyframes recording-wave {
  0%,
  100% {
    height: 4px;
  }
  50% {
    height: 16px;
  }
}
</style>
