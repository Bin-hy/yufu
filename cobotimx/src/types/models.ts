// ── API envelope ──
export type ApiResponse<T> = {
  message: string;
  data: T | null;
};

// ── Enums ──
export type ConversationType = 1 | 2;
export type MsgType = 1 | 2 | 3 | 4 | 5 | 6;
export type GroupRole = 0 | 1;

// ── Auth ──
export type LoginRequest = {
  username: string;
  password: string;
};

export type RegisterRequest = {
  username: string;
  nickname: string;
  password: string;
};

export type LoginResponse = {
  access_token: string;
  username: string;
  nickname: string;
};

// ── Conversation ──
export type ConversationSummary = {
  conversation_id: number;
  conversation_type: ConversationType;
  title: string | null;
  is_pinned: boolean;
  unread_count: number;
  last_message_preview: string | null;
};

export type ConversationDetail = {
  conversation_id: number;
  conversation_type: ConversationType;
  title: string | null;
  member_ids: number[];
};

export type CreateConversationRequest = {
  conversation_type: ConversationType;
  title?: string | null;
  member_ids: number[];
};

export type CreateConversationResponse = ConversationDetail;

export type ConversationListResponse = {
  limit: number;
  items: ConversationSummary[];
};

export type ClearUnreadResponse = {
  conversation_id: number;
  unread_count: number;
  last_read_message_id: number | null;
};

// ── Message ──
export type Message = {
  id: number;
  conversation_id: number;
  sender_id: number;
  msg_type: MsgType;
  content: string | null;
  extra: Record<string, unknown> | null;
  is_recalled: boolean;
  recalled_at: string | null;
  created_at: string;
};

export type SendMessageRequest = {
  msg_type: MsgType;
  content?: string | null;
  extra?: Record<string, unknown> | null;
  client_msg_id?: string | null;
};

export type SendMessageResponse = {
  message_id: number;
  conversation_id: number;
  msg_type: MsgType;
  client_msg_id: string | null;
  created_at: string;
};

export type GetMessagesResponse = {
  conversation_id: number;
  before_id: number | null;
  limit: number;
  has_more: boolean;
  items: Message[];
};

export type MediaExtra = {
  storage_path: string;
  width?: number;
  height?: number;
  duration?: number;
  size?: number;
  name?: string;
  client_msg_id?: string;
};

// ── Group ──
export type AddGroupMembersResponse = {
  conversation_id: number;
  added_member_ids: number[];
  skipped_member_ids: number[];
};

export type RemoveGroupMemberResponse = {
  conversation_id: number;
  user_id: number;
  removed: boolean;
};

export type LeaveGroupResponse = {
  conversation_id: number;
  user_id: number;
};

export type TransferGroupOwnerResponse = {
  conversation_id: number;
  old_owner_id: number;
  new_owner_id: number;
};

// ── File ──
export type UploadFileResponse = {
  path: string;
};

// ── WebSocket ──
export type WsClientEnvelope<T = unknown> = {
  event: string;
  seq?: string | null;
  data?: T | null;
};

export type WsServerEnvelope<T = unknown> = {
  event: string;
  status: number;
  message: string;
  data: T | null;
  seq: string | null;
  server_ts: number;
};

export type WsChatPushPayload = {
  message_id: number;
  from: number;
  to: number;
  msg_type: MsgType;
  conversation_id: number;
  content: string | null;
  extra: Record<string, unknown> | null;
  created_at: string;
};

export type WsGroupChatPushPayload = {
  message_id: number;
  conversation_id: number;
  from: number;
  msg_type: MsgType;
  content: string | null;
  extra: Record<string, unknown> | null;
  created_at: string;
};

export type WsMessageRecallPushPayload = {
  message_id: number;
  conversation_id: number;
  operator_id: number;
  recalled_at: string;
};

export type WsMessageDeletePushPayload = {
  message_id: number;
  conversation_id: number;
  operator_id: number;
  deleted_at: string;
};

export type WsConversationReadPushPayload = {
  conversation_id: number;
  reader_id: number;
  last_read_message_id: number | null;
  read_at: string;
};
