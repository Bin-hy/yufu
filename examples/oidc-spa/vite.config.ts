import { defineConfig } from 'vite'

export default defineConfig({
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // The browser bundle must never contain server-side reader credentials.
    sourcemap: false,
  },
  server: {
    port: 5173,
  },
})
