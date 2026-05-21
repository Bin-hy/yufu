import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import { resolve } from 'node:path'

export const ProjectRoot = resolve(import.meta.dirname, '../../../')

export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, process.cwd(), '')
    const COBOTRIX_URL = env.VITE_COBOTRIX_URL || 'http://localhost:4001'
    const COBOTSRV_URL = env.VITE_COBOTSRV_URL || 'http://localhost:4000'

    return {
        plugins: [vue()],
        base: '/',
        server: {
            proxy: {
                // Business API → cobotsrv (port 4001)
                '/devices': { target: COBOTRIX_URL, changeOrigin: true, secure: false },
                '/tasks': { target: COBOTRIX_URL, changeOrigin: true, secure: false },
                '/remote': { target: COBOTRIX_URL, changeOrigin: true, secure: false },
                '/hello': { target: COBOTRIX_URL, changeOrigin: true, secure: false },
                // OIDC + JWKS → cobotusr (port 4000)
                '/oidc': { target: COBOTSRV_URL, changeOrigin: true, secure: false },
                '/.well-known': { target: COBOTSRV_URL, changeOrigin: true, secure: false },
            },
        },
        build: {
            outDir: resolve(ProjectRoot, 'assets/cobotsrv'),
            emptyOutDir: true,
        },
        resolve: {
            alias: {
                '@': resolve(import.meta.dirname, './src'),
            },
        },
    }
})
