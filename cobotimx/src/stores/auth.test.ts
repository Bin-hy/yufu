// @vitest-environment jsdom
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useAuthStore } from "./auth";

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
  setActivePinia(createPinia());
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
  sessionStorage.clear();
});

it("loads a string OIDC subject as the existing numeric business user ID", async () => {
  localStorage.setItem("cobotimx.access_token", "test-access-token");
  localStorage.setItem("cobotimx.expires_at", String(Date.now() + 3_600_000));
  const fetchMock = vi.fn(async () => new Response(JSON.stringify({
    sub: "123",
    preferred_username: "alice",
  }), { status: 200 }));
  vi.stubGlobal("fetch", fetchMock);
  const store = useAuthStore();

  // completeSignIn starts the real UserInfo loader in the background.
  await expect(store.completeSignIn()).resolves.toBe(true);
  await vi.waitFor(() => {
    expect(store.userId).toBe(123);
    expect(store.username).toBe("alice");
  });
  expect(fetchMock).toHaveBeenCalledWith(
    expect.stringContaining("/api/v1/oidc/userinfo"),
    { headers: { Authorization: "Bearer test-access-token" } },
  );
});
