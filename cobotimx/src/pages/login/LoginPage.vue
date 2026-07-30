<script setup lang="ts">
import { useRoute } from "vue-router";
import { hasExplicitlySignedOut, redirectToLogin } from "@/auth/oidc";

const route = useRoute();

async function signIn() {
  const redirect = typeof route.query.redirect === "string" ? route.query.redirect : "/chat";
  await redirectToLogin(redirect.startsWith("/") ? redirect : "/chat");
}
</script>

<template>
  <main class="flex min-h-screen items-center justify-center bg-gray-100 p-6">
    <section class="w-full max-w-md rounded-xl bg-white p-8 text-center shadow-md">
      <h1 class="text-xl font-semibold text-gray-900">登录 CoBotrix IM</h1>
      <p class="mt-3 text-sm text-gray-600">
        {{ hasExplicitlySignedOut() ? "你已退出 IM。重新登录需要输入账号密码。" : "请使用统一账号服务登录 IM。" }}
      </p>
      <button
        class="mt-6 w-full rounded-lg bg-blue-600 py-2 text-white transition-colors hover:bg-blue-700"
        @click="signIn"
      >
        登录
      </button>
    </section>
  </main>
</template>
