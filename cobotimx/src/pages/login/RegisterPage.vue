<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const router = useRouter();
const auth = useAuthStore();

const username = ref("");
const nickname = ref("");
const password = ref("");
const password2 = ref("");
const error = ref("");
const loading = ref(false);

async function handleRegister() {
  if (!username.value.trim() || !nickname.value.trim() || !password.value) {
    error.value = "请填写所有字段";
    return;
  }
  if (password.value !== password2.value) {
    error.value = "两次密码不一致";
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    await auth.register({
      username: username.value.trim(),
      nickname: nickname.value.trim(),
      password: password.value,
    });
    router.push("/chat");
  } catch (e: unknown) {
    const msg = (e as { response?: { data?: { message?: string } } })?.response?.data?.message;
    error.value = msg || "注册失败";
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center bg-gray-100">
    <div class="w-full max-w-sm bg-white rounded-xl shadow-md p-8">
      <h1 class="text-2xl font-bold text-center mb-6">注册</h1>
      <form @submit.prevent="handleRegister" class="space-y-4">
        <div>
          <label class="block text-sm text-gray-600 mb-1">用户名</label>
          <input
            v-model="username"
            type="text"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="请输入用户名"
          />
        </div>
        <div>
          <label class="block text-sm text-gray-600 mb-1">昵称</label>
          <input
            v-model="nickname"
            type="text"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="请输入昵称"
          />
        </div>
        <div>
          <label class="block text-sm text-gray-600 mb-1">密码</label>
          <input
            v-model="password"
            type="password"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="请输入密码"
          />
        </div>
        <div>
          <label class="block text-sm text-gray-600 mb-1">确认密码</label>
          <input
            v-model="password2"
            type="password"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            placeholder="请确认密码"
          />
        </div>
        <p v-if="error" class="text-red-500 text-sm">{{ error }}</p>
        <button
          type="submit"
          :disabled="loading"
          class="w-full py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors"
        >
          {{ loading ? "注册中..." : "注册" }}
        </button>
      </form>
      <p class="text-center text-sm text-gray-500 mt-4">
        已有账号？
        <router-link to="/login" class="text-blue-600 hover:underline">登录</router-link>
      </p>
    </div>
  </div>
</template>
