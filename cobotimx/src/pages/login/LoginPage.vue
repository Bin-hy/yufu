<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = useRouter();
const auth = useAuthStore();

const username = ref("");
const password = ref("");
const error = ref("");
const loading = ref(false);

async function handleLogin() {
  if (!username.value.trim() || !password.value) {
    error.value = "请输入用户名和密码";
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    await auth.login({ username: username.value.trim(), password: password.value });
    router.push("/chat");
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    error.value = msg || "登录失败";
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-gray-100">
    <div class="w-full max-w-sm bg-white rounded-xl shadow-md p-8">
      <h1 class="text-2xl font-bold text-center mb-6">Cobotrix IM</h1>
      <form @submit.prevent="handleLogin" class="space-y-4">
        <div>
          <label class="block text-sm text-gray-600 mb-1">用户名</label>
          <input
            v-model="username"
            type="text"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="请输入用户名"
            autocomplete="username"
          />
        </div>
        <div>
          <label class="block text-sm text-gray-600 mb-1">密码</label>
          <input
            v-model="password"
            type="password"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="请输入密码"
            autocomplete="current-password"
          />
        </div>
        <p v-if="error" class="text-red-500 text-sm">{{ error }}</p>
        <button
          type="submit"
          :disabled="loading"
          class="w-full py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors"
        >
          {{ loading ? "登录中..." : "登录" }}
        </button>
      </form>
      <p class="text-center text-sm text-gray-500 mt-4">
        还没有账号？
        <router-link to="/register" class="text-blue-600 hover:underline">注册</router-link>
      </p>
    </div>
  </div>
</template>
