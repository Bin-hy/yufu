import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

export const ProjectRoot = resolve(import.meta.dirname, '../../../')

export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, process.cwd(), '')
    const API_TARGET = env.VITE_BASE_URL || 'http://localhost:3080'

    return {
        plugins: [vue()],
        base: '/',
        server: {
            proxy: {
                '/devices': { target: API_TARGET, changeOrigin: true, secure: false },
                '/tasks': { target: API_TARGET, changeOrigin: true, secure: false },
                '/hello': { target: API_TARGET, changeOrigin: true, secure: false },
            },
        },
        build: {
            outDir: resolve(ProjectRoot, 'assets/cobotlet'),
            emptyOutDir: true,
        },
        resolve: {
            alias: {
                '@': resolve(import.meta.dirname, './src'),
            },
        },
    }
})
