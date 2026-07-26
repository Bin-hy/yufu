import { defineConfig, loadEnv } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";

export default defineConfig(({ mode, command }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const proxyTarget = env.IM_API_PROXY_TARGET;

  if (command === "serve" && !proxyTarget) {
    throw new Error("IM_API_PROXY_TARGET is required for the Vite dev server");
  }

  return {
    plugins: [vue()],
    resolve: {
      alias: {
        "@": resolve(__dirname, "src"),
      },
    },
    server:
      command === "serve"
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
