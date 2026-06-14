import http from "@/http";
import type { ApiResponse } from "@/types/models";

export type UserVO = {
  id: number;
  username: string;
  nickname: string | null;
};

type ImUsersResponse = {
  users: UserVO[];
};

export const userApi = {
  fetchAll() {
    return http.get<ApiResponse<ImUsersResponse>>("/im/users", {
      params: { all: true },
    });
  },

  fetchByIds(ids: number[]) {
    return http.get<ApiResponse<ImUsersResponse>>("/im/users", {
      params: { ids: ids.join(",") },
    });
  },
};
