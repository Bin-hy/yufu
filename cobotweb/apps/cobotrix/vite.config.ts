import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

export const ProjectRoot = resolve(import.meta.dirname, '../../../')

export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, process.cwd(), '')
    // proxy to cobotsrv which forwards /mgmt/* to cobotrix
    const API_TARGET = env.VITE_COBOTSRV_URL || 'http://localhost:4000'

    return {
        plugins: [vue()],
        base: '/',
        server: {
            proxy: {
                '/mgmt': { target: API_TARGET, changeOrigin: true, secure: false },
                '/api': { target: API_TARGET, changeOrigin: true, secure: false },
                '/hello': { target: API_TARGET, changeOrigin: true, secure: false },
            },
        },
        build: {
            outDir: resolve(ProjectRoot, 'assets/cobotrix'),
            emptyOutDir: true,
        },
        resolve: {
            alias: {
                '@': resolve(import.meta.dirname, './src'),
            },
        },
    }
})
