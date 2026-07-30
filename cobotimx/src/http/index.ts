import axios from "axios";
import {
  clearAuth,
  getAccessToken,
  hasExplicitlySignedOut,
  refreshAccessToken,
  redirectToLogin,
} from "@/auth/oidc";

export const apiBase = import.meta.env.VITE_IM_API_BASE ?? "/api/v1";

const http = axios.create({
  baseURL: apiBase,
  timeout: 30_000,
  headers: { "Content-Type": "application/json" },
});

http.interceptors.request.use((config) => {
  const token = getAccessToken();
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

http.interceptors.response.use(
  (res) => res,
  async (error) => {
    const original = error.config;
    if (
      error.response?.status === 401 &&
      original &&
      !original._retry &&
      !hasExplicitlySignedOut()
    ) {
      original._retry = true;
      try {
        const token = await refreshAccessToken();
        if (token) {
          original.headers.Authorization = `Bearer ${token}`;
          return http(original);
        }
      } catch {
        // The common unauthorized path below clears local OIDC state and starts a new flow.
      }
      clearAuth();
      if (!hasExplicitlySignedOut()) {
        void redirectToLogin(window.location.pathname + window.location.search);
      }
    }
    return Promise.reject(error);
  },
);

export default http;
