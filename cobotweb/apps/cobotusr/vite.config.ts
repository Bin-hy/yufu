import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

// Walk up out of cobotweb/apps/cobotusr/ to the cobotrix repository root.
export const ProjectRoot = resolve(import.meta.dirname, '../../../');

export default defineConfig(({ mode }) => {
    // 读取环境变量
    const env = loadEnv(mode, process.cwd(), '')

    // 代理目标：VITE_BASE_URL 为绝对 URL（如 http://localhost:4000/api/v1）时直接使用；
    // 为相对路径（如 /api/v1）时，开发代理需要指向后端服务器地址。
    const API_TARGET = (env.VITE_BASE_URL?.startsWith('http') && env.VITE_BASE_URL) || 'http://127.0.0.1:4000'

    return {
        plugins: [
            vue(),
        ],
        base: '/',
        server: {
            proxy: {
                '/api/v1': {
                    target: API_TARGET,
                    changeOrigin: true,
                    secure: false,
                },
                '/oidc/token': {
                    target: API_TARGET,
                    changeOrigin: true,
                    secure: false,
                },
                '/oidc/revoke': {
                    target: API_TARGET,
                    changeOrigin: true,
                    secure: false,
                },
                '/oidc/introspect': {
                    target: API_TARGET,
                    changeOrigin: true,
                    secure: false,
                },
                '/oidc/authorize': {
                    target: API_TARGET,
                    changeOrigin: true,
                    secure: false,
                },
                '/.well-known': {
                    target: API_TARGET,
                    changeOrigin: true,
                    secure: false,
                },
            }
        },
        build: {
            outDir: resolve(ProjectRoot, "assets/cobotusr"),
        },
        resolve: {
            alias: {
                '@': resolve(import.meta.dirname, "./src"),
            },
        },
    }
})
