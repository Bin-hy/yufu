import { onUnmounted, type Ref } from "vue";
import { hasExplicitlySignedOut } from "@/auth/oidc";
import { useChatStore } from "@/stores/chat";

const CONVERSATION_POLL_INTERVAL = 15_000;
const DISCONNECTED_MESSAGE_POLL_INTERVAL = 15_000;

export function useChatPolling(wsConnected: Ref<boolean>) {
  const chat = useChatStore();
  let conversationTimer: ReturnType<typeof setInterval> | null = null;
  let messageTimer: ReturnType<typeof setInterval> | null = null;
  let loadingConversations = false;
  let loadingMessages = false;
  let destroyed = false;
  let conversationAbort: AbortController | null = null;
  let messageAbort: AbortController | null = null;

  async function pollConversations() {
    if (destroyed || hasExplicitlySignedOut() || loadingConversations) return;
    loadingConversations = true;
    conversationAbort = new AbortController();
    try {
      await chat.loadConversations(conversationAbort.signal);
    } catch {
      // Keep polling; transient HTTP failures are handled by the next interval.
    } finally {
      conversationAbort = null;
      loadingConversations = false;
    }
  }

  async function pollActiveMessagesWhenDisconnected() {
    if (destroyed || hasExplicitlySignedOut() || wsConnected.value || loadingMessages) return;
    const conversationId = chat.activeConversationId;
    if (!conversationId) return;

    loadingMessages = true;
    messageAbort = new AbortController();
    try {
      await chat.loadMessages(conversationId, undefined, messageAbort.signal);
    } catch {
      // WebSocket is the primary channel. Polling is only a fallback.
    } finally {
      messageAbort = null;
      loadingMessages = false;
    }
  }

  function start() {
    if (destroyed) return;
    if (!conversationTimer) {
      conversationTimer = setInterval(pollConversations, CONVERSATION_POLL_INTERVAL);
    }
    if (!messageTimer) {
      messageTimer = setInterval(
        pollActiveMessagesWhenDisconnected,
        DISCONNECTED_MESSAGE_POLL_INTERVAL,
      );
    }
  }

  function stop() {
    destroyed = true;
    conversationAbort?.abort();
    messageAbort?.abort();
    conversationAbort = null;
    messageAbort = null;
    if (conversationTimer) {
      clearInterval(conversationTimer);
      conversationTimer = null;
    }
    if (messageTimer) {
      clearInterval(messageTimer);
      messageTimer = null;
    }
  }

  onUnmounted(stop);

  return { start, stop };
}
