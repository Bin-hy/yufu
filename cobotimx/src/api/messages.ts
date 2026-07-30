import http from "@/http";
import type { ApiResponse, GetMessagesResponse, SendMessageRequest, SendMessageResponse } from "@/types/models";

export const messageApi = {
  list(conversationId: number, beforeId?: number, limit = 50, signal?: AbortSignal) {
    return http.get<ApiResponse<GetMessagesResponse>>(
      `/im/conversations/${conversationId}/messages`,
      { params: { before_id: beforeId, limit }, signal },
    );
  },

  send(conversationId: number, data: SendMessageRequest) {
    return http.post<ApiResponse<SendMessageResponse>>(
      `/im/conversations/${conversationId}/messages`,
      data,
    );
  },

  recall(conversationId: number, messageId: number) {
    return http.patch<ApiResponse<{ message_id: number; conversation_id: number; recalled_at: string }>>(
      `/im/conversations/${conversationId}/messages`,
      { message_id: messageId },
    );
  },

  delete(conversationId: number, messageId: number) {
    return http.delete<ApiResponse<{ message_id: number; conversation_id: number; deleted_at: string }>>(
      `/im/conversations/${conversationId}/messages`,
      { data: { message_id: messageId } },
    );
  },
};
