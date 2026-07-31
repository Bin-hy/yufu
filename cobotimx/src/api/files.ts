import http from "@/http";
import { apiBase } from "@/http";
import { getAccessToken } from "@/auth/oidc";
import type { ApiResponse, UploadFileResponse } from "@/types/models";

export const fileApi = {
  upload(file: File) {
    const form = new FormData();
    form.append("file", file);
    return http.post<ApiResponse<UploadFileResponse>>("/im/files", form, {
      headers: { "Content-Type": "multipart/form-data" },
      timeout: 120_000,
    });
  },

  getUrl(path: string): string {
    const token = getAccessToken() ?? "";
    return `${apiBase}/im/files?path=${encodeURIComponent(path)}&access_token=${encodeURIComponent(token)}`;
  },
};
