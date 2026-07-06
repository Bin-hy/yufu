import { createRouter, createWebHistory } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/login",
      name: "Login",
      component: () => import("@/pages/login/LoginPage.vue"),
      meta: { guest: true },
    },
    {
      path: "/register",
      name: "Register",
      component: () => import("@/pages/login/RegisterPage.vue"),
      meta: { guest: true },
    },
    {
      path: "/",
      redirect: "/chat",
    },
    {
      path: "/chat",
      name: "Chat",
      component: () => import("@/pages/chat/ChatLayout.vue"),
      meta: { requiresAuth: true },
    },
    {
      path: "/:pathMatch(.*)*",
      redirect: "/chat",
    },
  ],
});

router.beforeEach((to, _from, next) => {
  const auth = useAuthStore();
  if (to.meta.requiresAuth && !auth.isLoggedIn) {
    next("/login");
  } else if (to.meta.guest && auth.isLoggedIn) {
    next("/chat");
  } else {
    next();
  }
});

export default router;
