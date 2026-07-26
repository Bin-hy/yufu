import http from "@/http";
import type { ApiResponse, LoginRequest, LoginResponse, RegisterRequest } from "@/types/models";

export const authApi = {
  login(data: LoginRequest) {
    return http.post<LoginResponse>("/login", data);
  },

  register(data: RegisterRequest) {
    return http.post<LoginResponse>("/register", data);
  },

  refresh() {
    return http.post<LoginResponse>("/refresh", null, { withCredentials: true });
  },

  logout() {
    return http.post<ApiResponse<null>>("/logout", null, { withCredentials: true });
  },
};
