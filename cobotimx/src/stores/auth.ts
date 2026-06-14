import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { authApi } from "@/api/auth";
import type { LoginRequest, RegisterRequest } from "@/types/models";

function parseJwtPayload(token: string): Record<string, unknown> | null {
  try {
    const parts = token.split(".");
    if (parts.length !== 3) return null;
    const raw = atob(parts[1].replace(/-/g, "+").replace(/_/g, "/"));
    return JSON.parse(decodeURIComponent(escape(raw)));
  } catch {
    return null;
  }
}

function extractUserId(token: string): number {
  const payload = parseJwtPayload(token);
  if (payload) {
    // AccessTokenClaims uses "sub" (Rust IdType serialized as string), not "id"
    const sub = payload.sub;
    if (typeof sub === "string") {
      const id = Number(sub);
      if (!isNaN(id)) return id;
    }
  }
  return 0;
}

export const useAuthStore = defineStore("auth", () => {
  const accessToken = ref(sessionStorage.getItem("access_token") || "");
  const username = ref(sessionStorage.getItem("username") || "");
  const nickname = ref(sessionStorage.getItem("nickname") || "");
  const userId = ref(extractUserId(accessToken.value));

  const isLoggedIn = computed(() => !!accessToken.value);

  function setAuth(token: string, uname: string, nick: string) {
    accessToken.value = token;
    username.value = uname;
    nickname.value = nick;
    userId.value = extractUserId(token);
    sessionStorage.setItem("access_token", token);
    sessionStorage.setItem("username", uname);
    sessionStorage.setItem("nickname", nick);
  }

  function clearAuth() {
    accessToken.value = "";
    username.value = "";
    nickname.value = "";
    userId.value = 0;
    sessionStorage.removeItem("access_token");
    sessionStorage.removeItem("username");
    sessionStorage.removeItem("nickname");
  }

  async function login(data: LoginRequest) {
    const res = await authApi.login(data);
    const d = res.data;
    setAuth(d.access_token, d.username, d.nickname);
    return d;
  }

  async function register(data: RegisterRequest) {
    const res = await authApi.register(data);
    const d = res.data;
    setAuth(d.access_token, d.username, d.nickname);
    return d;
  }

  async function logout() {
    try {
      await authApi.logout();
    } finally {
      clearAuth();
    }
  }

  return { accessToken, username, nickname, userId, isLoggedIn, setAuth, clearAuth, login, register, logout };
});
