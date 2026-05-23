import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The SPA is served at `/admin/app` by the Rust tileserver in
// production (see crates/turbo-tiles-bin/src/main.rs). Vite's `base`
// option is set accordingly so asset URLs work both behind the
// gateway and standalone.
export default defineConfig({
  plugins: [react()],
  base: "/admin/app/",
  server: {
    port: 5173,
    proxy: {
      // During `npm run dev`, forward API calls to the local tileserver
      // so the SPA can hit /admin/api/* and /v1/* without CORS pain.
      "/admin/api": "http://localhost:8090",
      "/v1": "http://localhost:8090",
    },
  },
  build: {
    outDir: "dist",
    sourcemap: true,
  },
});
