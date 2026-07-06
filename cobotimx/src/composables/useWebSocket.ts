import { ref, onUnmounted } from "vue";
import { useChatStore } from "@/stores/chat";
import { useAuthStore } from "@/stores/auth";
import type {
  WsServerEnvelope,
  WsChatPushPayload,
  WsGroupChatPushPayload,
  WsMessageRecallPushPayload,
  WsMessageDeletePushPayload,
  WsConversationReadPushPayload,
} from "@/types/models";

const HEARTBEAT_INTERVAL = 25_000;
const RECONNECT_DELAY = 3_000;

function getWsUrl(): string {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  const token = sessionStorage.getItem("access_token") || "";
  return `${proto}//${location.host}/api/v1/im/ws?access_token=${encodeURIComponent(token)}`;
}

export function useWebSocket() {
  const connected = ref(false);
  let ws: WebSocket | null = null;
  let heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let seqCounter = 0;
  let destroyed = false;

  function connect() {
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
    if (!sessionStorage.getItem("access_token")) return;

    ws = new WebSocket(getWsUrl());

    ws.onopen = () => {
      connected.value = true;
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
      scheduleReconnect();
    };

    ws.onerror = () => {
      ws?.close();
    };
  }

  function disconnect() {
    destroyed = true;
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
    reconnectTimer = setTimeout(connect, RECONNECT_DELAY);
  }

  function clearReconnect() {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
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
        const preview = p.content ?? "[媒体]";
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
        const preview = p.content ?? "[媒体]";
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
