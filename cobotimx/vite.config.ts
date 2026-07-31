/// <reference types="vitest/config" />
import { defineConfig, loadEnv } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";

export default defineConfig(({ mode, command, isPreview }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const proxyTarget = env.IM_API_PROXY_TARGET;
  // Vite >= 6 exposes isPreview on the config callback; `vite preview` also
  // runs with command "serve", so npm_lifecycle_event heuristics are
  // unreliable (e.g. Docker CMD ["vite", "preview"]).
  const isDevServer = command === "serve" && !isPreview && mode !== "test";

  if (isDevServer && !proxyTarget) {
    throw new Error("IM_API_PROXY_TARGET is required for the Vite dev server");
  }

  return {
    plugins: [vue()],
    test: {
      setupFiles: ["src/test-setup.ts"],
    },
    resolve: {
      alias: {
        "@": resolve(__dirname, "src"),
      },
    },
    server:
      isDevServer
        ? {
            port: 5174,
            proxy: {
              "/api": {
                target: proxyTarget,
                changeOrigin: true,
                ws: true,
              },
            },
          }
        : undefined,
  };
});
