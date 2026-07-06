import http from "@/http";
import type {
  AddGroupMembersResponse,
  ApiResponse,
  LeaveGroupResponse,
  RemoveGroupMemberResponse,
  TransferGroupOwnerResponse,
} from "@/types/models";

export const groupApi = {
  addMembers(conversationId: number, memberIds: number[]) {
    return http.post<ApiResponse<AddGroupMembersResponse>>(
      `/im/conversations/${conversationId}/members`,
      { member_ids: memberIds },
    );
  },

  removeMember(conversationId: number, userId: number) {
    return http.delete<ApiResponse<RemoveGroupMemberResponse>>(
      `/im/conversations/${conversationId}/members/${userId}`,
    );
  },

  leave(conversationId: number) {
    return http.delete<ApiResponse<LeaveGroupResponse>>(
      `/im/conversations/${conversationId}/members/me`,
    );
  },

  transferOwner(conversationId: number, newOwnerId: number) {
    return http.patch<ApiResponse<TransferGroupOwnerResponse>>(
      `/im/conversations/${conversationId}/owner`,
      { new_owner_id: newOwnerId },
    );
  },
};
