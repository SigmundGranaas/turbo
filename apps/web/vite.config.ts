import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// The turbomap-web WASM module is a `file:` dependency, so it resolves through
// node_modules; its wasm-bindgen glue loads the `.wasm` via
// `new URL('…_bg.wasm', import.meta.url)`, which Vite serves/fingerprints
// natively. `exclude` keeps esbuild's dep-optimiser from trying to pre-bundle
// the wasm glue (it must run as a real ES module so `import.meta.url` resolves).
// In dev, proxy /api → the prod backend server-to-server so the browser sees a
// same-origin request (the live backend's CORS allowlist doesn't include the
// Vite dev origin). Pair with VITE_API_BASE='' (.env.development) so the client
// issues relative /api/... URLs. Prod builds use the absolute API base + real CORS.
export default defineConfig({
  plugins: [react()],
  optimizeDeps: { exclude: ['turbomap-web'] },
  // Vitest unit tests (pure logic + store behaviour). jsdom gives trackImport a
  // DOMParser; the WASM renderer is never imported by tested code.
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.{ts,tsx}'],
  },
  server: {
    port: 5173,
    proxy: {
      // Modulith + places + routing are under /api; the tileserver is served at
      // top-level /v1, /fonts, /sprite by the prod ingress (see infra ingress).
      '/api': { target: 'https://kart-api.sandring.no', changeOrigin: true, secure: true },
      '/v1': { target: 'https://kart-api.sandring.no', changeOrigin: true, secure: true },
      '/fonts': { target: 'https://kart-api.sandring.no', changeOrigin: true, secure: true },
      '/sprite': { target: 'https://kart-api.sandring.no', changeOrigin: true, secure: true },
    },
  },
});
