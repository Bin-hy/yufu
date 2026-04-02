import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

export const ProjectRoot = resolve(import.meta.dirname, '../');

export default defineConfig(({ mode }) => {
    // 读取环境变量
    const env = loadEnv(mode, process.cwd(), '')

    // 目标地址，优先使用环境变量，否则使用默认值
    const API_TARGET = env.VITE_BASE_URL || 'http://localhost:4000'

    return {
        plugins: [
            vue(),
        ],
        base: './',
        server: {
            proxy: {
                '/api/v1': {
                    target: API_TARGET,
                    changeOrigin: true,
                    secure: false,
                    rewrite: (path) => path.replace(/^\/api\/v1/, ''),
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
            outDir: resolve(ProjectRoot, "assets/cobotsrv"),
        },
        resolve: {
            alias: {
                '@': resolve("./src"),
            },
        },
    }
})
