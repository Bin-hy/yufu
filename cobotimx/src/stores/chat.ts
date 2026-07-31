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

  async function loadConversations(signal?: AbortSignal) {
    const res = await conversationApi.list(50, signal);
    if (signal?.aborted) return;
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

  // ── Active conversation detail (loaded on select) ──
  const activeConversationMembers = ref<number[]>([]);
  const activeConversationOwnerId = ref<number | null>(null);

  async function loadActiveConversationDetail() {
    activeConversationMembers.value = [];
    activeConversationOwnerId.value = null;
    if (!activeConversationId.value) return;
    try {
      const res = await conversationApi.detail(activeConversationId.value);
      if (res.data.data) {
        activeConversationMembers.value = res.data.data.member_ids;
        activeConversationOwnerId.value = res.data.data.owner_id;
      }
    } catch (e) {
      console.error("Failed to load conversation detail:", e);
    }
  }

  // ── Messages ──
  const messages = ref<Message[]>([]);
  const hasMore = ref(true);
  const loadingMessages = ref(false);

  async function loadMessages(conversationId: number, beforeId?: number, signal?: AbortSignal) {
    loadingMessages.value = true;
    try {
      const res = await messageApi.list(conversationId, beforeId, 50, signal);
      if (signal?.aborted) return;
      // 切换会话后迟到的响应不得覆盖当前会话的消息
      if (conversationId !== activeConversationId.value) return;
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

  // 轮询专用合并：按消息 id 去重，追加新消息并同步已有消息的撤回等状态。
  // 不重置 hasMore、不清空已加载的历史分页，也不闪烁全局 loadingMessages。
  async function mergePolledMessages(conversationId: number, signal?: AbortSignal) {
    const res = await messageApi.list(conversationId, undefined, 50, signal);
    if (signal?.aborted) return;
    if (conversationId !== activeConversationId.value) return;
    const items = res.data.data?.items;
    if (!items) return;
    const byId = new Map(messages.value.map((m) => [m.id, m]));
    const fresh: Message[] = [];
    for (const item of items) {
      const existing = byId.get(item.id);
      if (existing) {
        existing.content = item.content;
        existing.extra = item.extra;
        existing.is_recalled = item.is_recalled;
        existing.recalled_at = item.recalled_at;
      } else {
        fresh.push(item);
      }
    }
    if (fresh.length) {
      messages.value = [...messages.value, ...fresh].sort((a, b) => a.id - b.id);
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
    activeConversationMembers.value = [];
    activeConversationOwnerId.value = null;
  }

  return {
    conversations,
    activeConversationId,
    activeConversation,
    activeConversationMembers,
    activeConversationOwnerId,
    loadActiveConversationDetail,
    loadConversations,
    upsertConversationFromPush,
    messages,
    hasMore,
    loadingMessages,
    loadMessages,
    mergePolledMessages,
    addMessage,
    addSentMessage,
    markRecalled,
    removeMessage,
    clearMessages,
  };
});
