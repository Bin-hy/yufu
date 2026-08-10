<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from "vue";
import { useMediaRecorder } from "@/composables/useMediaRecorder";

const props = defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  close: [];
  recorded: [file: File, durationSec: number];
}>();

type ModalState = "preview" | "recording" | "review";

const recorder = useMediaRecorder("video");
const modalState = ref<ModalState>("preview");

const previewVideo = ref<HTMLVideoElement | null>(null);
const reviewVideo = ref<HTMLVideoElement | null>(null);

const recordDuration = ref(0);
const recordDisplay = computed(() => {
  const totalSec = Math.floor(recordDuration.value);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${String(min).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
});

const pendingFile = ref<File | null>(null);

// Watch visibility to start/stop camera
watch(
  () => props.visible,
  async (v) => {
    if (v) {
      modalState.value = "preview";
      recordDuration.value = 0;
      pendingFile.value = null;
      await startPreview();
    } else {
      recorder.cancel();
      modalState.value = "preview";
    }
  }
);

async function startPreview() {
  const stream = await recorder.requestPreviewStream();
  if (stream && previewVideo.value) {
    previewVideo.value.srcObject = stream;
    await previewVideo.value.play().catch(() => {
      // Autoplay blocked — user will see muted video anyway
    });
  } else if (recorder.error.value) {
    alert(recorder.error.value);
  }
}

function startRecording() {
  if (recorder.state.value !== "idle" && recorder.state.value !== "stopped") return;

  recordDuration.value = 0;
  modalState.value = "recording";

  recorder.start().then(() => {
    if (recorder.state.value === "denied" && recorder.error.value) {
      alert(recorder.error.value);
      modalState.value = "preview";
    }
  });

  // Poll duration from the composable
  const timer = setInterval(() => {
    if (modalState.value !== "recording") {
      clearInterval(timer);
      return;
    }
    recordDuration.value = recorder.durationSec.value;
  }, 100);
}

async function stopRecording() {
  if (recorder.state.value !== "recording") return;

  const file = await recorder.stop();
  if (file) {
    pendingFile.value = file;
    // Show preview
    const url = recorder.getBlobUrl();
    if (url && reviewVideo.value) {
      reviewVideo.value.src = url;
    }
    modalState.value = "review";
  } else {
    // Too short
    alert("录制时间太短");
    modalState.value = "preview";
    // Re-start preview
    await startPreview();
  }
}

function sendRecording() {
  if (!pendingFile.value) return;
  const dur = Math.round(recordDuration.value);
  emit("recorded", pendingFile.value, dur);
  emit("close");
}

function reRecord() {
  pendingFile.value = null;
  modalState.value = "preview";
  // Re-start preview stream since stop() cleaned it up
  startPreview();
}

function closeModal() {
  if (modalState.value === "recording") return; // prevent closing while recording
  recorder.cancel();
  emit("close");
}

// Keyboard: Escape to close (not during recording)
function onKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape" && modalState.value !== "recording") {
    closeModal();
  }
}

watch(
  () => props.visible,
  (v) => {
    if (v) {
      window.addEventListener("keydown", onKeyDown);
    } else {
      window.removeEventListener("keydown", onKeyDown);
    }
  }
);

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
  recorder.cancel();
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      style="backdrop-filter: blur(4px)"
      @click.self="closeModal"
    >
      <!-- Modal container -->
      <div
        class="bg-white rounded-2xl shadow-2xl overflow-hidden flex flex-col"
        style="width: 560px; max-width: 95vw; max-height: 90vh"
      >
        <!-- Header -->
        <div class="flex items-center justify-between px-5 py-3 border-b border-gray-200">
          <h3 class="text-lg font-semibold text-gray-800">录制视频</h3>
          <button
            v-if="modalState !== 'recording'"
            class="text-gray-400 hover:text-gray-600 text-2xl leading-none p-1 transition-colors"
            @click="closeModal"
            title="关闭"
          >
            ✕
          </button>
        </div>

        <!-- Video area -->
        <div class="relative bg-black flex items-center justify-center" style="min-height: 360px">
          <!-- Preview state: live camera -->
          <video
            v-show="modalState === 'preview' || modalState === 'recording'"
            ref="previewVideo"
            autoplay
            muted
            playsinline
            class="w-full object-cover"
            :style="modalState === 'preview' || modalState === 'recording' ? 'transform: scaleX(-1);' : ''"
            style="max-height: 480px"
          />

          <!-- Review state: recorded playback -->
          <video
            v-show="modalState === 'review'"
            ref="reviewVideo"
            controls
            autoplay
            playsinline
            class="w-full object-cover"
            style="max-height: 480px"
          />

          <!-- Recording indicator -->
          <div
            v-if="modalState === 'recording'"
            class="absolute top-4 right-4 flex items-center gap-2 bg-black/60 rounded-full px-3 py-1.5"
          >
            <span class="w-3 h-3 rounded-full bg-red-500 recording-pulse-dot" />
            <span class="text-white text-sm font-mono">{{ recordDisplay }}</span>
          </div>
        </div>

        <!-- Controls -->
        <div class="flex items-center justify-center gap-4 px-5 py-4 border-t border-gray-200">
          <!-- Preview: start recording -->
          <template v-if="modalState === 'preview'">
            <button
              class="flex items-center gap-2 px-6 py-3 bg-red-500 text-white rounded-full hover:bg-red-600 transition-colors font-medium shadow-lg"
              @click="startRecording"
            >
              <span class="w-6 h-6 rounded-full border-4 border-white" />
              <span>开始录制</span>
            </button>
          </template>

          <!-- Recording: stop -->
          <template v-if="modalState === 'recording'">
            <button
              class="flex items-center gap-2 px-6 py-3 bg-red-500 text-white rounded-full hover:bg-red-600 transition-colors font-medium shadow-lg"
              @click="stopRecording"
            >
              <span class="w-4 h-4 bg-white rounded-sm" />
              <span>停止录制</span>
            </button>
          </template>

          <!-- Review: send or re-record -->
          <template v-if="modalState === 'review'">
            <button
              class="px-5 py-2.5 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-colors font-medium"
              @click="reRecord"
            >
              重录
            </button>
            <button
              class="px-6 py-2.5 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors font-medium shadow"
              @click="sendRecording"
            >
              发送
            </button>
          </template>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
@keyframes recording-pulse-dot {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.3;
  }
}

.recording-pulse-dot {
  animation: recording-pulse-dot 1s ease-in-out infinite;
}
</style>
