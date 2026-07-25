<script setup lang="ts">
import { useRoute } from "vue-router";
import { redirectToLogin } from "@/auth/oidc";

const route = useRoute();

async function signIn() {
  const redirect = typeof route.query.redirect === "string" ? route.query.redirect : "/chat";
  await redirectToLogin(redirect.startsWith("/") ? redirect : "/chat");
}
</script>

<template>
  <main class="flex min-h-screen items-center justify-center bg-gray-100 p-6">
    <section class="w-full max-w-md rounded-xl bg-white p-8 text-center shadow-md">
      <h1 class="text-xl font-semibold text-gray-900">Sign in to CoBotrix IM</h1>
      <p class="mt-3 text-sm text-gray-600">
        You have signed out of IM. Select sign in to continue with the unified account service.
      </p>
      <button
        class="mt-6 w-full rounded-lg bg-blue-600 py-2 text-white transition-colors hover:bg-blue-700"
        @click="signIn"
      >
        Sign in
      </button>
    </section>
  </main>
</template>
