import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The turbomap-web WASM module is a `file:` dependency, so it resolves through
// node_modules; its wasm-bindgen glue loads the `.wasm` via
// `new URL('…_bg.wasm', import.meta.url)`, which Vite serves/fingerprints
// natively. `exclude` keeps esbuild's dep-optimiser from trying to pre-bundle
// the wasm glue (it must run as a real ES module so `import.meta.url` resolves).
export default defineConfig({
  plugins: [react()],
  optimizeDeps: { exclude: ['turbomap-web'] },
  server: { port: 5173 },
});
