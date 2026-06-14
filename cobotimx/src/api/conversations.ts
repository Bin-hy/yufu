import http from "@/http";
import type {
  ApiResponse,
  ClearUnreadResponse,
  ConversationDetail,
  ConversationListResponse,
  CreateConversationRequest,
  CreateConversationResponse,
} from "@/types/models";

export const conversationApi = {
  list(limit = 50) {
    return http.get<ApiResponse<ConversationListResponse>>("/im/conversations", {
      params: { limit },
    });
  },

  create(data: CreateConversationRequest) {
    return http.post<ApiResponse<CreateConversationResponse>>("/im/conversations", data);
  },

  detail(id: number) {
    return http.get<ApiResponse<ConversationDetail>>(`/im/conversations/${id}`);
  },

  remove(id: number) {
    return http.delete<ApiResponse<{ conversation_id: number; deleted: boolean }>>(
      `/im/conversations/${id}`,
    );
  },

  members(id: number) {
    return http.get<ApiResponse<{ conversation_id: number; member_ids: number[] }>>(
      `/im/conversations/${id}/members`,
    );
  },

  clearUnread(id: number) {
    return http.put<ApiResponse<ClearUnreadResponse>>(`/im/conversations/${id}/read-state`);
  },
};
