<script setup lang="ts">
import { useChatStore } from "@/stores/chat";
import { useAuthStore } from "@/stores/auth";
import { useNicknames } from "@/composables/useNicknames";
import { conversationApi } from "@/api/conversations";
import type { ConversationDetail } from "@/types/models";
import { ref, watch } from "vue";

const emit = defineEmits<{ (e: "open-settings"): void }>();

const chat = useChatStore();
const auth = useAuthStore();
const { get: getNick } = useNicknames();

const detail = ref<ConversationDetail | null>(null);
const showDetail = ref(false);

watch(
  () => chat.activeConversationId,
  async (id) => {
    if (!id) {
      detail.value = null;
      return;
    }
    try {
      // don't await, just load detail in background
      const res = await conversationApi.detail(id);
      detail.value = res.data.data;
    } catch {
      detail.value = null;
    }
  },
  { immediate: true },
);

function title(): string {
  if (!chat.activeConversation) return "";
  const conv = chat.activeConversation;
  if (conv.title) return conv.title;
  if (conv.conversation_type === 1 && detail.value) {
    const other = detail.value.member_ids.find(
      (id) => id !== auth.userId,
    );
    return other ? getNick(other) : `会话 #${conv.conversation_id}`;
  }
  return `会话 #${conv.conversation_id}`;
}

function subtitle(): string {
  if (!detail.value) return "";
  return `${detail.value.member_ids.length} 人`;
}

async function handleDelete() {
  if (!chat.activeConversationId) return;
  if (!confirm("确定要删除/退出此会话？")) return;
  try {
    await conversationApi.remove(chat.activeConversationId);
    chat.activeConversationId = null;
    chat.clearMessages();
    await chat.loadConversations();
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    alert(msg || "删除会话失败");
  }
}
</script>

<template>
  <div class="flex items-center justify-between px-4 py-3 border-b border-gray-200 bg-white">
    <div class="flex-1 min-w-0">
      <div class="flex items-center gap-2">
        <h2 class="font-semibold truncate">{{ title() }}</h2>
        <span v-if="showDetail" class="text-xs text-gray-400">({{ subtitle() }})</span>
      </div>
    </div>
    <div class="flex items-center gap-1">
      <button
        @click="showDetail = !showDetail"
        class="px-2 py-1 text-sm text-gray-500 hover:bg-gray-100 rounded transition-colors"
        title="切换详情"
      >
        i
      </button>
      <button
        v-if="chat.activeConversation?.conversation_type === 2"
        @click="emit('open-settings')"
        class="px-2 py-1 text-sm text-gray-500 hover:bg-gray-100 rounded transition-colors"
        title="群设置"
      >
        ⚙
      </button>
      <button
        @click="handleDelete"
        class="px-2 py-1 text-sm text-gray-500 hover:text-red-600 hover:bg-gray-100 rounded transition-colors"
        title="删除/退出"
      >
        🗑
      </button>
    </div>
  </div>
</template>
