<script setup lang="ts">
import { ref } from "vue";
import { useChatStore } from "@/stores/chat";
import { messageApi } from "@/api/messages";
import { fileApi } from "@/api/files";
import type { MsgType, MediaExtra } from "@/types/models";
import VoiceRecordButton from "@/components/recording/VoiceRecordButton.vue";
import VideoRecorderModal from "@/components/recording/VideoRecorderModal.vue";
import PhotoCaptureModal from "@/components/recording/PhotoCaptureModal.vue";

const chat = useChatStore();
const text = ref("");
const sending = ref(false);
const showVideoRecorder = ref(false);
const showPhotoCapture = ref(false);

// Emoji palette
const showEmoji = ref(false);
const emojis = ["😀", "😂", "❤️", "👍", "🎉", "🔥", "😢", "😡", "👋", "💯", "✨", "🙏"];

function insertEmoji(emoji: string) {
  text.value += emoji;
  showEmoji.value = false;
}

function generateClientMsgId(): string {
  return crypto.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

async function sendText() {
  const content = text.value.trim();
  if (!content || !chat.activeConversationId || sending.value) return;
  sending.value = true;
  const req = {
    msg_type: 1 as MsgType,
    content,
    client_msg_id: generateClientMsgId(),
  };
  try {
    const res = await messageApi.send(chat.activeConversationId, req);
    if (res.data.data) {
      chat.addSentMessage(res.data.data, req);
      chat.upsertConversationFromPush(chat.activeConversationId!, content, false);
    }
    text.value = "";
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "发送失败");
  } finally {
    sending.value = false;
  }
}

async function sendMedia(file: File, msgType: MsgType, extraFields?: Partial<MediaExtra>) {
  if (!chat.activeConversationId) return;
  sending.value = true;
  try {
    const uploadRes = await fileApi.upload(file);
    const path = uploadRes.data.data?.path;
    if (!path) throw new Error("上传失败");

    const extra: MediaExtra = {
      storage_path: path,
      size: file.size,
      name: file.name,
      ...extraFields,
    };
    if (msgType === 2) {
      try {
        const bmp = await createImageBitmap(file);
        extra.width = bmp.width;
        extra.height = bmp.height;
        bmp.close();
      } catch { /* ignore */ }
    }

    const req = {
      msg_type: msgType,
      extra: extra as Record<string, unknown>,
      client_msg_id: generateClientMsgId(),
    };
    const res = await messageApi.send(chat.activeConversationId, req);
    if (res.data.data) {
      chat.addSentMessage(res.data.data, req);
      const previewText =
        msgType === 2 ? "[图片]" :
        msgType === 3 ? "[语音]" :
        msgType === 4 ? "[视频]" :
        "[媒体]";
      chat.upsertConversationFromPush(chat.activeConversationId!, previewText, false);
    }
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "发送媒体失败");
  } finally {
    sending.value = false;
  }
}

function onFileChange(event: Event, type: MsgType) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) sendMedia(file, type);
  input.value = "";
}

function onVoiceRecorded(file: File, durationSec: number) {
  sendMedia(file, 3, { duration: durationSec });
}

function onVideoRecorded(file: File, durationSec: number) {
  showVideoRecorder.value = false;
  sendMedia(file, 4, { duration: durationSec });
}

function onPhotoCaptured(file: File) {
  showPhotoCapture.value = false;
  sendMedia(file, 2);
}

function onRecordError(msg: string) {
  alert(msg);
}
</script>

<template>
  <div class="border-t border-gray-200 bg-white px-4 py-3">
    <!-- Emoji picker -->
    <div v-if="showEmoji" class="flex flex-wrap gap-1 mb-2 p-2 bg-gray-50 rounded-lg">
      <button
        v-for="e in emojis"
        :key="e"
        @click="insertEmoji(e)"
        class="text-xl hover:bg-gray-200 rounded p-1 transition-colors"
      >
        {{ e }}
      </button>
    </div>

    <!-- Toolbar: icon + text below -->
    <div class="flex items-center gap-1 mb-2">
      <button
        @click="showEmoji = !showEmoji"
        class="flex flex-col items-center px-2 py-1 rounded hover:bg-gray-100 transition-colors text-gray-600"
        title="表情"
      >
        <span class="text-lg leading-none mb-0.5">😊</span>
        <span class="text-xs leading-tight">表情</span>
      </button>
      <button
        class="flex flex-col items-center px-2 py-1 rounded hover:bg-gray-100 transition-colors text-gray-600 disabled:opacity-50"
        title="拍照"
        :disabled="sending"
        @click="showPhotoCapture = true"
      >
        <span class="text-lg leading-none mb-0.5">📷</span>
        <span class="text-xs leading-tight">拍照</span>
      </button>
      <label class="flex flex-col items-center px-2 py-1 rounded hover:bg-gray-100 transition-colors cursor-pointer text-gray-600" title="发送文件">
        <span class="text-lg leading-none mb-0.5">📎</span>
        <span class="text-xs leading-tight">文件</span>
        <input type="file" class="hidden" @change="onFileChange($event, 5)" />
      </label>
      <VoiceRecordButton
        :disabled="sending"
        @recorded="onVoiceRecorded"
        @error="onRecordError"
      />
      <button
        class="flex flex-col items-center px-2 py-1 rounded hover:bg-gray-100 transition-colors text-gray-600 disabled:opacity-50"
        title="录制视频"
        :disabled="sending"
        @click="showVideoRecorder = true"
      >
        <span class="text-lg leading-none mb-0.5">📹</span>
        <span class="text-xs leading-tight">录制视频</span>
      </button>
    </div>

    <!-- Input row -->
    <div class="flex items-center gap-2">
      <textarea
        v-model="text"
        class="flex-1 resize-none rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
        rows="2"
        placeholder="输入消息..."
        @keydown.enter.exact.prevent="sendText"
      />
      <button
        @click="sendText"
        :disabled="sending || !text.trim()"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors text-sm font-medium"
      >
        发送
      </button>
    </div>

    <!-- Video recorder modal -->
    <VideoRecorderModal
      :visible="showVideoRecorder"
      @close="showVideoRecorder = false"
      @recorded="onVideoRecorded"
    />

    <!-- Photo capture modal -->
    <PhotoCaptureModal
      :visible="showPhotoCapture"
      @close="showPhotoCapture = false"
      @captured="onPhotoCaptured"
    />
  </div>
</template>
