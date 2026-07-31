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
      const result = await refreshAccessToken();
      if (result.status === "ok") {
        original.headers.Authorization = `Bearer ${result.token}`;
        return http(original);
      }
      if (result.status === "invalid") {
        // Credentials are conclusively rejected: clear local OIDC state and
        // start a new sign-in flow.
        clearAuth();
        if (!hasExplicitlySignedOut()) {
          void redirectToLogin(window.location.pathname + window.location.search);
        }
      }
      // "error" is transient (network/server): keep the stored tokens and
      // only reject the current request.
    }
    return Promise.reject(error);
  },
);

export default http;
