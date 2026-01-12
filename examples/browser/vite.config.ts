import { defineConfig } from 'vite'

export default defineConfig({
  server: {
    port: 8080,
    headers: {
      // Required for WebGPU and WASM
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Opener-Policy': 'same-origin'
    }
  },
  build: {
    target: 'esnext'
  },
  optimizeDeps: {
    exclude: ['@mlc-ai/web-llm']
  }
})
