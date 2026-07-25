import { createRouter, createWebHistory } from "vue-router";
import { hasExplicitlySignedOut, redirectToLogin } from "@/auth/oidc";
import { useAuthStore } from "@/stores/auth";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/oauth/callback",
      name: "OAuthCallback",
      component: () => import("@/pages/oauth/OAuthCallbackPage.vue"),
      meta: { requiresAuth: false },
    },
    {
      path: "/login",
      name: "Login",
      component: () => import("@/pages/login/LoginPage.vue"),
      meta: { requiresAuth: false },
    },
    { path: "/register", redirect: "/login" },
    { path: "/", redirect: "/chat" },
    {
      path: "/chat",
      name: "Chat",
      component: () => import("@/pages/chat/ChatLayout.vue"),
      meta: { requiresAuth: true },
    },
    { path: "/:pathMatch(.*)*", redirect: "/chat" },
  ],
});

router.beforeEach(async (to) => {
  if (to.meta.requiresAuth !== true) return true;

  const auth = useAuthStore();
  if (await auth.ensureAuthenticated()) return true;

  if (hasExplicitlySignedOut()) {
    return { name: "Login", query: { redirect: to.fullPath } };
  }

  await redirectToLogin(to.fullPath);
  return false;
});

export default router;
