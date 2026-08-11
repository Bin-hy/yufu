<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useAuthStore } from "@/stores/auth";
import { useNicknames } from "@/composables/useNicknames";
import { messageApi } from "@/api/messages";
import { fileApi } from "@/api/files";
import { formatMessageTime } from "@/utils/time";
import type { Message } from "@/types/models";

const props = defineProps<{ message: Message }>();
const auth = useAuthStore();
const { get: getNick } = useNicknames();

const isMine = computed(() => props.message.sender_id === auth.userId);
const time = computed(() => formatMessageTime(props.message.created_at));

const mediaPath = computed(() => {
  const extra = props.message.extra as Record<string, unknown> | null;
  if (!extra?.storage_path) return null;
  return fileApi.getUrl(String(extra.storage_path));
});

const audioDuration = computed(() => {
  const extra = props.message.extra as Record<string, unknown> | null;
  const d = extra?.duration;
  if (typeof d === "number" && d > 0) return Math.round(d);
  return 0;
});

const audioRef = ref<HTMLAudioElement | null>(null);
const isPlaying = ref(false);
const previewImage = ref<string | null>(null);

function togglePlay() {
  const audio = audioRef.value;
  if (!audio) return;
  if (audio.paused) {
    audio.play().catch(() => {});
    isPlaying.value = true;
  } else {
    audio.pause();
    isPlaying.value = false;
  }
}

function onAudioEnded() {
  isPlaying.value = false;
}

function openPreview(url: string) {
  previewImage.value = url;
}

function closePreview() {
  previewImage.value = null;
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") closePreview();
}

watch(previewImage, (v) => {
  if (v) window.addEventListener("keydown", onKeydown);
  else window.removeEventListener("keydown", onKeydown);
});

// Width proportional to duration (min 60px, max 200px)
const bubbleMinWidth = computed(() => {
  if (audioDuration.value <= 0) return "60px";
  const w = Math.min(200, Math.max(60, audioDuration.value * 12 + 40));
  return `${w}px`;
});

async function handleRecall() {
  if (!confirm("撤回这条消息？")) return;
  try {
    await messageApi.recall(props.message.conversation_id, props.message.id);
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "撤回失败");
  }
}

async function handleDelete() {
  if (!confirm("删除这条消息？")) return;
  try {
    await messageApi.delete(props.message.conversation_id, props.message.id);
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "删除失败");
  }
}
</script>

<template>
  <div :class="['flex', isMine ? 'justify-end' : 'justify-start']">
    <div
      :class="[
        'max-w-[70%] rounded-2xl px-4 py-2 text-sm relative group',
        isMine ? 'bg-blue-600 text-white' : 'bg-white text-gray-800 shadow-sm border border-gray-100',
      ]"
    >
      <!-- Sender -->
      <p
        v-if="!isMine"
        class="text-xs font-medium mb-1"
        :class="isMine ? 'text-blue-100' : 'text-blue-600'"
      >
        {{ getNick(message.sender_id) }}
      </p>

      <!-- Recalled -->
      <p v-if="message.is_recalled" class="italic opacity-60">消息已撤回</p>

      <!-- Content -->
      <template v-else>
        <!-- Text -->
        <p v-if="message.msg_type === 1" class="whitespace-pre-wrap break-words">{{ message.content }}</p>

        <!-- Image -->
        <div v-else-if="message.msg_type === 2 && mediaPath">
          <img
            :src="mediaPath"
            class="max-w-60 max-h-60 rounded-lg object-cover cursor-pointer hover:opacity-90 transition-opacity"
            loading="lazy"
            @click="openPreview(mediaPath)"
          />
          <p v-if="message.content" class="text-xs mt-1 opacity-80">{{ message.content }}</p>
        </div>

        <!-- Video -->
        <div v-else-if="message.msg_type === 4 && mediaPath">
          <video :src="mediaPath" controls class="max-w-60 max-h-60 rounded-lg" />
          <p v-if="message.content" class="text-xs mt-1 opacity-80">{{ message.content }}</p>
        </div>

        <!-- Audio (WeChat-style: icon + seconds, tap to play/pause) -->
        <div
          v-else-if="message.msg_type === 3 && mediaPath"
          class="flex items-center gap-2 py-1 cursor-pointer select-none"
          :style="{ minWidth: bubbleMinWidth }"
          @click="togglePlay"
        >
          <!-- Hidden native audio element -->
          <audio ref="audioRef" :src="mediaPath" class="hidden" @ended="onAudioEnded" />

          <!-- Speaker icon with playing animation -->
          <span class="text-lg flex-shrink-0" :class="{ 'animate-pulse': isPlaying }">
            🔊
          </span>

          <!-- Duration in seconds -->
          <span
            class="text-xs whitespace-nowrap flex-shrink-0"
            :class="isMine ? 'text-blue-100' : 'text-gray-500'"
          >
            {{ audioDuration || '--' }}''
          </span>

          <!-- Playing wave bars -->
          <span v-if="isPlaying" class="flex items-center gap-0.5 ml-1">
            <span
              v-for="i in 3"
              :key="i"
              class="wave-bar w-1 rounded-full inline-block"
              :class="isMine ? 'bg-blue-200' : 'bg-blue-400'"
              :style="{ animationDelay: `${(i - 1) * 0.15}s` }"
            />
          </span>
        </div>

        <p v-if="message.msg_type === 3 && message.content" class="text-xs mt-1 opacity-80">{{ message.content }}</p>

        <!-- File -->
        <div v-else-if="message.msg_type === 5">
          <a
            v-if="mediaPath"
            :href="mediaPath"
            target="_blank"
            class="underline"
            :class="isMine ? 'text-white' : 'text-blue-600'"
          >
            📎 {{ (message.extra as Record<string, unknown>)?.name || "下载文件" }}
          </a>
          <p v-else class="opacity-60">[文件]</p>
          <p v-if="message.content" class="text-xs mt-1 opacity-80">{{ message.content }}</p>
        </div>

        <!-- System -->
        <p v-else-if="message.msg_type === 6" class="italic opacity-60">
          {{ message.content }}
        </p>
      </template>

      <!-- Time & Actions -->
      <div class="flex items-center justify-end gap-2 mt-1">
        <span class="text-xs opacity-60">{{ time }}</span>
        <div v-if="isMine && !message.is_recalled" class="hidden group-hover:flex items-center gap-1">
          <button @click="handleRecall" class="text-xs underline opacity-60 hover:opacity-100">撤回</button>
          <button @click="handleDelete" class="text-xs underline opacity-60 hover:opacity-100">删除</button>
        </div>
      </div>
    </div>
  </div>

  <!-- Image lightbox -->
  <Teleport to="body">
    <div
      v-if="previewImage"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 cursor-pointer"
      @click="closePreview"
    >
      <img
        :src="previewImage"
        class="max-w-[95vw] max-h-[95vh] object-contain rounded-lg"
        @click.stop
      />
      <button
        class="absolute top-4 right-4 text-white text-3xl opacity-60 hover:opacity-100 transition-opacity"
        @click="closePreview"
      >
        ✕
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.wave-bar {
  height: 10px;
  animation: voice-wave 0.6s ease-in-out infinite;
}

@keyframes voice-wave {
  0%,
  100% {
    height: 4px;
  }
  50% {
    height: 14px;
  }
}
</style>
