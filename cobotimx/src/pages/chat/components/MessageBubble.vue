<script setup lang="ts">
import { computed } from "vue";
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
          <img :src="mediaPath" class="max-w-60 max-h-60 rounded-lg object-cover" loading="lazy" />
          <p v-if="message.content" class="text-xs mt-1 opacity-80">{{ message.content }}</p>
        </div>

        <!-- Video -->
        <div v-else-if="message.msg_type === 4 && mediaPath">
          <video :src="mediaPath" controls class="max-w-60 max-h-60 rounded-lg" />
          <p v-if="message.content" class="text-xs mt-1 opacity-80">{{ message.content }}</p>
        </div>

        <!-- Audio -->
        <div v-else-if="message.msg_type === 3 && mediaPath">
          <audio :src="mediaPath" controls class="max-w-48" />
          <p v-if="message.content" class="text-xs mt-1 opacity-80">{{ message.content }}</p>
        </div>

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
</template>
