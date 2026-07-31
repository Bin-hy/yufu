import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  clearAuth as clearOidcAuth,
  fetchUserInfo,
  getAccessToken,
  redirectToIdpLogout,
  refreshAccessToken,
  revokeAndClearAuth,
} from "@/auth/oidc";

function extractUserId(token: string): number {
  try {
    const payload = token.split(".")[1];
    if (!payload) return 0;
    const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
    const decoded = JSON.parse(atob(normalized)) as { sub?: unknown };
    const id = Number(decoded.sub);
    return Number.isFinite(id) ? id : 0;
  } catch {
    return 0;
  }
}

export const useAuthStore = defineStore("auth", () => {
  const accessToken = ref(getAccessToken() ?? "");
  const username = ref("");
  const nickname = ref("");
  const userId = ref(extractUserId(accessToken.value));
  const isLoggedIn = computed(() => Boolean(accessToken.value));

  // Skip userinfo requests when a fresh profile was loaded recently, so every
  // route navigation does not fire a fire-and-forget loadUserInfo call.
  const USER_INFO_TTL_MS = 60_000;
  let userInfoFetchedAt = 0;

  function syncToken(token: string | null = getAccessToken()) {
    accessToken.value = token ?? "";
    userId.value = token ? extractUserId(token) : 0;
  }

  async function loadUserInfo(force = false): Promise<boolean> {
    if (!force && userInfoFetchedAt && Date.now() - userInfoFetchedAt < USER_INFO_TTL_MS) {
      return true;
    }
    const user = await fetchUserInfo();
    if (!user) return false;
    username.value = user.preferred_username ?? "";
    nickname.value = user.nickname ?? user.name ?? username.value;
    const id = Number(user.sub);
    if (Number.isFinite(id)) userId.value = id;
    userInfoFetchedAt = Date.now();
    return true;
  }

  async function ensureAuthenticated(): Promise<boolean> {
    let token = getAccessToken();
    if (!token) {
      const result = await refreshAccessToken();
      token = result.status === "ok" ? result.token : null;
    }
    syncToken(token);
    if (!token) {
      username.value = "";
      nickname.value = "";
      return false;
    }
    // The access token is the authentication result. Profile retrieval is
    // supplementary and must not discard a valid OIDC session if it is
    // temporarily unavailable (for example during a cross-origin retry).
    void loadUserInfo();
    return true;
  }

  async function completeSignIn(): Promise<boolean> {
    syncToken();
    if (!accessToken.value) return false;
    void loadUserInfo(true);
    return true;
  }

  function clearAuth() {
    clearOidcAuth();
    syncToken(null);
    username.value = "";
    nickname.value = "";
    userInfoFetchedAt = 0;
  }

  async function logout() {
    try {
      await revokeAndClearAuth();
    } finally {
      syncToken(null);
      username.value = "";
      nickname.value = "";
      userInfoFetchedAt = 0;
      // A revoke request can fail independently (for example due to a network
      // error), but explicit logout must still clear the IdP browser session.
      redirectToIdpLogout();
    }
  }

  return {
    accessToken,
    username,
    nickname,
    userId,
    isLoggedIn,
    syncToken,
    ensureAuthenticated,
    completeSignIn,
    clearAuth,
    logout,
  };
});
