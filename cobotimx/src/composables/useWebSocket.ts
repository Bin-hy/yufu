import { ref, onUnmounted } from "vue";
import { useChatStore } from "@/stores/chat";
import { useAuthStore } from "@/stores/auth";
import { getAccessToken, hasExplicitlySignedOut, refreshAccessToken } from "@/auth/oidc";
import { apiBase } from "@/http";
import type {
  WsServerEnvelope,
  WsChatPushPayload,
  WsGroupChatPushPayload,
  WsMessageRecallPushPayload,
  WsMessageDeletePushPayload,
  WsConversationReadPushPayload,
} from "@/types/models";

const HEARTBEAT_INTERVAL = 25_000;
const RECONNECT_MIN_DELAY = 1_000;
const RECONNECT_MAX_DELAY = 30_000;

function getWsUrl(): string {
  const apiUrl = new URL(apiBase, window.location.origin);
  apiUrl.protocol = apiUrl.protocol === "https:" ? "wss:" : "ws:";
  apiUrl.pathname = `${apiUrl.pathname.replace(/\/$/, "")}/im/ws`;
  apiUrl.searchParams.set("access_token", getAccessToken() ?? "");
  return apiUrl.toString();
}

export function useWebSocket() {
  const connected = ref(false);
  let ws: WebSocket | null = null;
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let connectPromise: Promise<void> | null = null;
  let seqCounter = 0;
  let destroyed = false;
  let reconnectAttempts = 0;
  let refreshOnReconnect = false;

  async function connectInner() {
    if (destroyed) return;
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
    // The server may reject a locally unexpired token. Force one refresh on
    // the first reconnect of an outage instead of spinning on the stale
    // token; later retries follow the backoff rhythm without refreshing.
    if (refreshOnReconnect) {
      refreshOnReconnect = false;
      await refreshAccessToken();
    }
    if (!getAccessToken()) {
      const result = await refreshAccessToken();
      if (result.status !== "ok") {
        if (!hasExplicitlySignedOut()) scheduleReconnect();
        return;
      }
    }
    if (destroyed) return;

    ws = new WebSocket(getWsUrl());

    ws.onopen = () => {
      connected.value = true;
      reconnectAttempts = 0;
      refreshOnReconnect = false;
      startHeartbeat();
    };

    ws.onmessage = (event) => {
      try {
        const envelope: WsServerEnvelope = JSON.parse(event.data);
        handleMessage(envelope);
      } catch {
        // ignore malformed messages
      }
    };

    ws.onclose = () => {
      connected.value = false;
      stopHeartbeat();
      // Only the first close of an outage triggers a forced token refresh.
      if (reconnectAttempts === 0) refreshOnReconnect = true;
      scheduleReconnect();
    };

    ws.onerror = () => {
      ws?.close();
    };
  }

  function connect() {
    if (!connectPromise) {
      connectPromise = connectInner().finally(() => {
        connectPromise = null;
      });
    }
    return connectPromise;
  }

  function disconnect() {
    destroyed = true;
    connectPromise = null;
    stopHeartbeat();
    clearReconnect();
    if (ws) {
      ws.onclose = null;
      ws.close();
      ws = null;
    }
    connected.value = false;
  }

  function sendPing() {
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const seq = `ping-${++seqCounter}`;
    ws.send(JSON.stringify({ event: "heartbeat.ping", seq, data: {} }));
  }

  function startHeartbeat() {
    stopHeartbeat();
    sendPing();
    heartbeatTimer = setInterval(sendPing, HEARTBEAT_INTERVAL);
  }

  function stopHeartbeat() {
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
      heartbeatTimer = null;
    }
  }

  function scheduleReconnect() {
    if (destroyed) return;
    clearReconnect();
    // Exponential backoff: 1s, 2s, 4s, ... capped at 30s; reset on connect.
    const delay = Math.min(RECONNECT_MIN_DELAY * 2 ** reconnectAttempts, RECONNECT_MAX_DELAY);
    reconnectAttempts += 1;
    reconnectTimer = setTimeout(connect, delay);
  }

  function clearReconnect() {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  }

  function previewForType(msgType: unknown): string {
    const t = msgType as number;
    switch (t) {
      case 2: return "[图片]";
      case 3: return "[语音]";
      case 4: return "[视频]";
      case 5: return "[文件]";
      case 6: return "[系统消息]";
      default: return "[媒体]";
    }
  }

  function handleMessage(env: WsServerEnvelope) {
    const chat = useChatStore();
    const auth = useAuthStore();
    const data = env.data as Record<string, unknown> | null;
    if (!data) return;

    switch (env.event) {
      case "chat.push": {
        const p = data as unknown as WsChatPushPayload;
        if (p.from === auth.userId) break; // already added locally on send
        const preview = p.content ?? previewForType(p.msg_type);
        chat.upsertConversationFromPush(p.conversation_id, preview, true);
        if (chat.activeConversationId === p.conversation_id) {
          chat.addMessage({
            id: p.message_id,
            conversation_id: p.conversation_id,
            sender_id: p.from,
            msg_type: p.msg_type as never,
            content: p.content,
            extra: p.extra,
            is_recalled: false,
            recalled_at: null,
            created_at: p.created_at,
          });
        }
        break;
      }
      case "group.push": {
        const p = data as unknown as WsGroupChatPushPayload;
        if (p.from === auth.userId) break; // already added locally on send
        const preview = p.content ?? previewForType(p.msg_type);
        chat.upsertConversationFromPush(p.conversation_id, preview, true);
        if (chat.activeConversationId === p.conversation_id) {
          chat.addMessage({
            id: p.message_id,
            conversation_id: p.conversation_id,
            sender_id: p.from,
            msg_type: p.msg_type as never,
            content: p.content,
            extra: p.extra,
            is_recalled: false,
            recalled_at: null,
            created_at: p.created_at,
          });
        }
        break;
      }
      case "message.recall.push": {
        const p = data as unknown as WsMessageRecallPushPayload;
        chat.markRecalled(p.message_id);
        break;
      }
      case "message.delete.push": {
        const p = data as unknown as WsMessageDeletePushPayload;
        chat.removeMessage(p.message_id);
        break;
      }
      case "conversation.read.push": {
        // Optional: update read status UI
        break;
      }
    }
  }

  onUnmounted(disconnect);

  return { connected, connect, disconnect };
}
