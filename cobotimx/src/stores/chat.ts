import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { conversationApi } from "@/api/conversations";
import { messageApi } from "@/api/messages";
import { useAuthStore } from "@/stores/auth";
import type {
  ConversationSummary,
  ConversationDetail,
  Message,
  SendMessageRequest,
  SendMessageResponse,
} from "@/types/models";

export const useChatStore = defineStore("chat", () => {
  // ── Conversations ──
  const conversations = ref<ConversationSummary[]>([]);
  const activeConversationId = ref<number | null>(null);

  const activeConversation = computed(() =>
    conversations.value.find((c) => c.conversation_id === activeConversationId.value) ?? null,
  );

  async function loadConversations() {
    const res = await conversationApi.list();
    if (res.data.data) {
      conversations.value = res.data.data.items;
    }
  }

  function upsertConversationFromPush(
    conversationId: number,
    preview: string,
    incrementUnread: boolean,
  ) {
    const existing = conversations.value.find((c) => c.conversation_id === conversationId);
    if (existing) {
      existing.last_message_preview = preview;
      if (incrementUnread && existing.conversation_id !== activeConversationId.value) {
        existing.unread_count += 1;
      }
    } else {
      loadConversations();
    }
  }

  // ── Messages ──
  const messages = ref<Message[]>([]);
  const hasMore = ref(true);
  const loadingMessages = ref(false);

  async function loadMessages(conversationId: number, beforeId?: number) {
    loadingMessages.value = true;
    try {
      const res = await messageApi.list(conversationId, beforeId);
      if (res.data.data) {
        const result = res.data.data;
        if (beforeId) {
          messages.value = [...result.items, ...messages.value];
        } else {
          messages.value = result.items;
        }
        hasMore.value = result.has_more;
      }
    } finally {
      loadingMessages.value = false;
    }
  }

  function addMessage(msg: Message) {
    // Dedup by message id or client_msg_id
    const exists = messages.value.some(
      (m) => m.id === msg.id || (msg.id === 0 && m.extra && (m.extra as Record<string, unknown>).client_msg_id === (msg.extra as Record<string, unknown> | null)?.client_msg_id),
    );
    if (!exists) {
      messages.value.push(msg);
    }
  }

  function addSentMessage(resp: SendMessageResponse, req: SendMessageRequest) {
    const auth = useAuthStore();
    addMessage({
      id: resp.message_id,
      conversation_id: resp.conversation_id,
      sender_id: auth.userId,
      msg_type: resp.msg_type,
      content: req.content ?? null,
      extra: req.extra ?? null,
      is_recalled: false,
      recalled_at: null,
      created_at: resp.created_at,
    });
  }

  function markRecalled(messageId: number) {
    const msg = messages.value.find((m) => m.id === messageId);
    if (msg) {
      msg.is_recalled = true;
      msg.content = null;
      msg.extra = null;
    }
  }

  function removeMessage(messageId: number) {
    messages.value = messages.value.filter((m) => m.id !== messageId);
  }

  function clearMessages() {
    messages.value = [];
    hasMore.value = true;
  }

  return {
    conversations,
    activeConversationId,
    activeConversation,
    loadConversations,
    upsertConversationFromPush,
    messages,
    hasMore,
    loadingMessages,
    loadMessages,
    addMessage,
    addSentMessage,
    markRecalled,
    removeMessage,
    clearMessages,
  };
});
