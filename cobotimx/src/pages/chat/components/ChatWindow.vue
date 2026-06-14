<script setup lang="ts">
import { useChatStore } from "@/stores/chat";
import ChatHeader from "./ChatHeader.vue";
import MessageBubble from "./MessageBubble.vue";
import MessageInput from "./MessageInput.vue";
import GroupSettings from "./GroupSettings.vue";
import { ref, watch, nextTick } from "vue";

const chat = useChatStore();
const showGroupSettings = ref(false);
const messagesContainer = ref<HTMLElement | null>(null);

async function loadMore() {
  const msgs = chat.messages;
  if (!msgs.length || !chat.activeConversationId || !chat.hasMore) return;
  const oldest = msgs[0];
  await chat.loadMessages(chat.activeConversationId, oldest.id);
}

function onScroll() {
  const el = messagesContainer.value;
  if (!el) return;
  if (el.scrollTop <= 50 && chat.hasMore && !chat.loadingMessages) {
    loadMore();
  }
}

// Auto-scroll to bottom when new messages arrive
watch(
  () => chat.messages.length,
  () => {
    nextTick(() => {
      const el = messagesContainer.value;
      if (el) {
        el.scrollTop = el.scrollHeight;
      }
    });
  },
);
</script>

<template>
  <template v-if="chat.activeConversation">
    <ChatHeader @open-settings="showGroupSettings = true" />
    <div
      ref="messagesContainer"
      class="flex-1 overflow-y-auto px-4 py-3 space-y-2 bg-gray-50"
      @scroll="onScroll"
    >
      <div v-if="chat.loadingMessages" class="text-center text-xs text-gray-400 py-2">
        加载中...
      </div>
      <div v-else-if="chat.hasMore" class="text-center text-xs text-gray-400 py-2">
        上滑加载更多
      </div>
      <MessageBubble
        v-for="msg in chat.messages"
        :key="msg.id"
        :message="msg"
      />
      <div v-if="chat.messages.length === 0 && !chat.loadingMessages" class="text-center text-gray-400 text-sm py-12">
        暂无消息，打个招呼吧！
      </div>
    </div>
    <MessageInput />
    <!-- Group settings panel -->
    <GroupSettings
      v-if="showGroupSettings && chat.activeConversation?.conversation_type === 2"
      @close="showGroupSettings = false"
    />
  </template>
  <template v-else>
    <div class="flex-1 flex items-center justify-center text-gray-400">
      <div class="text-center">
        <p class="text-5xl mb-4">💬</p>
        <p class="text-lg">选择一个会话开始聊天</p>
      </div>
    </div>
  </template>
</template>
