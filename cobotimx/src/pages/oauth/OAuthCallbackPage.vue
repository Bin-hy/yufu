<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { handleCallback, redirectToLogin } from "@/auth/oidc";
import { useAuthStore } from "@/stores/auth";

const route = useRoute();
const router = useRouter();
const auth = useAuthStore();
const error = ref("");

onMounted(async () => {
  const code = typeof route.query.code === "string" ? route.query.code : undefined;
  const state = typeof route.query.state === "string" ? route.query.state : undefined;
  const oauthError = typeof route.query.error === "string" ? route.query.error : undefined;

  if (oauthError) {
    error.value = `授权失败：${oauthError}`;
    return;
  }
  if (!code || !state) {
    error.value = "授权回调缺少 code 或 state 参数。";
    return;
  }

  try {
    const destination = await handleCallback(code, state);
    if (!(await auth.completeSignIn())) {
      auth.clearAuth();
      throw new Error("无法加载已登录用户的信息。");
    }
    await router.replace(destination);
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "登录失败。";
  }
});

async function retry() {
  await redirectToLogin("/chat");
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-gray-100 p-6">
    <div class="w-full max-w-md rounded-xl bg-white p-8 shadow-md">
      <p v-if="!error" class="text-center text-gray-600">正在登录…</p>
      <template v-else>
        <h1 class="mb-2 text-lg font-semibold text-gray-900">无法登录</h1>
        <p class="mb-5 break-words text-sm text-red-600">{{ error }}</p>
        <button
          class="w-full rounded-lg bg-blue-600 py-2 text-white transition-colors hover:bg-blue-700"
          @click="retry"
        >
          重试
        </button>
      </template>
    </div>
  </div>
</template>
