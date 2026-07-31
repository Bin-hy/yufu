<script setup lang="ts">
import { onMounted } from "vue";
import { useChatStore } from "@/stores/chat";
import { useChatPolling } from "@/composables/useChatPolling";
import { useWebSocket } from "@/composables/useWebSocket";
import { useNicknames } from "@/composables/useNicknames";
import ConversationList from "./components/ConversationList.vue";
import ChatWindow from "./components/ChatWindow.vue";

const chat = useChatStore();
const { connected, connect } = useWebSocket();
const { start: startPolling } = useChatPolling(connected);
const { loadAll } = useNicknames();

onMounted(async () => {
  await Promise.all([chat.loadConversations(), loadAll()]);
  connect();
  startPolling();
});
</script>

<template>
  <div class="h-screen flex bg-white">
    <!-- Sidebar -->
    <div class="w-80 flex-shrink-0 border-r border-gray-200">
      <ConversationList />
    </div>
    <!-- Main chat area -->
    <div class="flex-1 flex flex-col min-w-0">
      <ChatWindow />
    </div>
  </div>
</template>
