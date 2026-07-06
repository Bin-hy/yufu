import http from "@/http";
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
    const token = sessionStorage.getItem("access_token") || "";
    return `/api/v1/im/files?path=${encodeURIComponent(path)}&access_token=${encodeURIComponent(token)}`;
  },
};
