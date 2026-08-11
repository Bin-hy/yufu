<script setup lang="ts">
import { ref, watch, onUnmounted, nextTick } from "vue";

const props = defineProps<{ visible: boolean }>();

const emit = defineEmits<{
  close: [];
  captured: [file: File];
}>();

type ModalState = "preview" | "captured";

const modalState = ref<ModalState>("preview");
const previewVideo = ref<HTMLVideoElement | null>(null);
const stream = ref<MediaStream | null>(null);
const error = ref<string | null>(null);
const photoDataUrl = ref<string | null>(null);
const photoBlob = ref<Blob | null>(null);

// Watch visibility to start/stop camera
watch(
  () => props.visible,
  async (v) => {
    if (v) {
      modalState.value = "preview";
      photoDataUrl.value = null;
      photoBlob.value = null;
      error.value = null;
      await startCamera();
    } else {
      stopCamera();
    }
  }
);

async function startCamera() {
  try {
    const ms = await navigator.mediaDevices.getUserMedia({
      video: { facingMode: "user", width: { ideal: 1280 }, height: { ideal: 720 } },
    });
    stream.value = ms;
    await nextTick();
    if (previewVideo.value) {
      previewVideo.value.srcObject = ms;
      await previewVideo.value.play().catch(() => {});
    }
  } catch (err: unknown) {
    const domErr = err as DOMException;
    if (domErr.name === "NotAllowedError") {
      error.value = "无法访问摄像头，请在浏览器设置中允许摄像头权限";
    } else if (domErr.name === "NotFoundError") {
      error.value = "未检测到摄像头设备";
    } else {
      error.value = `摄像头启动失败: ${domErr.message || String(err)}`;
    }
  }
}

function stopCamera() {
  if (stream.value) {
    stream.value.getTracks().forEach((t) => t.stop());
    stream.value = null;
  }
}

function takePhoto() {
  const video = previewVideo.value;
  if (!video || !video.videoWidth) return;

  const canvas = document.createElement("canvas");
  canvas.width = video.videoWidth;
  canvas.height = video.videoHeight;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  // Canvas draws the un-flipped frame regardless of CSS transform
  ctx.drawImage(video, 0, 0);

  photoDataUrl.value = canvas.toDataURL("image/jpeg", 0.9);
  canvas.toBlob(
    (blob) => {
      if (blob) photoBlob.value = blob;
    },
    "image/jpeg",
    0.9
  );

  modalState.value = "captured";
  // Keep camera running for potential retake
}

function sendPhoto() {
  if (!photoBlob.value) return;
  const file = new File([photoBlob.value], `photo_${formatTimestamp()}.jpg`, {
    type: "image/jpeg",
  });
  emit("captured", file);
  emit("close");
}

function retakePhoto() {
  photoDataUrl.value = null;
  photoBlob.value = null;
  modalState.value = "preview";
}

function closeModal() {
  stopCamera();
  emit("close");
}

// Keyboard: Escape to close
function onKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape") closeModal();
}

watch(
  () => props.visible,
  (v) => {
    if (v) window.addEventListener("keydown", onKeyDown);
    else window.removeEventListener("keydown", onKeyDown);
  }
);

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
  stopCamera();
});

function formatTimestamp(): string {
  const now = new Date();
  const y = now.getFullYear();
  const mo = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  const h = String(now.getHours()).padStart(2, "0");
  const mi = String(now.getMinutes()).padStart(2, "0");
  const s = String(now.getSeconds()).padStart(2, "0");
  return `${y}${mo}${d}_${h}${mi}${s}`;
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      style="backdrop-filter: blur(4px)"
      @click.self="closeModal"
    >
      <div
        class="bg-white rounded-2xl shadow-2xl overflow-hidden flex flex-col"
        style="width: 560px; max-width: 95vw; max-height: 90vh"
      >
        <!-- Header -->
        <div class="flex items-center justify-between px-5 py-3 border-b border-gray-200">
          <h3 class="text-lg font-semibold text-gray-800">拍照</h3>
          <button
            class="text-gray-400 hover:text-gray-600 text-2xl leading-none p-1 transition-colors"
            @click="closeModal"
            title="关闭"
          >
            ✕
          </button>
        </div>

        <!-- Camera / Photo area -->
        <div class="relative bg-black flex items-center justify-center" style="min-height: 360px">
          <!-- Error state -->
          <div v-if="error" class="text-white text-center p-8">
            <p class="text-lg mb-2">{{ error }}</p>
            <button
              class="px-4 py-2 bg-white text-gray-800 rounded-lg hover:bg-gray-200 transition-colors"
              @click="closeModal"
            >
              关闭
            </button>
          </div>

          <!-- Preview: live camera (mirrored) -->
          <video
            v-show="modalState === 'preview' && !error"
            ref="previewVideo"
            autoplay
            muted
            playsinline
            class="w-full object-cover"
            style="transform: scaleX(-1); max-height: 480px"
          />

          <!-- Captured: static photo -->
          <img
            v-show="modalState === 'captured'"
            :src="photoDataUrl || undefined"
            class="w-full object-contain"
            style="max-height: 480px"
          />
        </div>

        <!-- Controls -->
        <div class="flex items-center justify-center gap-4 px-5 py-4 border-t border-gray-200">
          <!-- Preview: take photo -->
          <template v-if="modalState === 'preview' && !error">
            <button
              class="w-16 h-16 rounded-full border-4 border-gray-300 bg-white hover:border-gray-400 transition-colors shadow-lg flex items-center justify-center"
              @click="takePhoto"
              title="拍照"
            >
              <span class="w-12 h-12 rounded-full bg-gray-800" />
            </button>
          </template>

          <!-- Captured: send or retake -->
          <template v-if="modalState === 'captured'">
            <button
              class="px-5 py-2.5 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 transition-colors font-medium"
              @click="retakePhoto"
            >
              重拍
            </button>
            <button
              class="px-6 py-2.5 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors font-medium shadow"
              @click="sendPhoto"
            >
              发送
            </button>
          </template>
        </div>
      </div>
    </div>
  </Teleport>
</template>
