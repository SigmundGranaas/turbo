# Image for the Turbo web app — served at kart.sandring.no as `turbo-web`
# (replaces the old Flutter web image; same nginx-on-:8080 contract, so the k8s
# Deployment/Service/Ingress in infra/k8s/base/web.yaml are unchanged).
#
# Build context is `apps/` — it needs both `web/` (the React app) and
# `turbomap/` (the wgpu renderer compiled to WASM, a `file:` dep of the web app,
# so the `web/ ↔ turbomap/` sibling layout must be preserved in the image).
#
#   docker build -f apps/web/Dockerfile -t turbo-web apps

# ── Stage 1: turbomap-core/-engine → wasm-bindgen pkg ───────────────────────
FROM rust:1.92-bookworm AS wasm
RUN rustup target add wasm32-unknown-unknown \
 && cargo install wasm-pack --locked
WORKDIR /src
COPY turbomap turbomap
# Release build → wasm-opt'd, smaller .wasm. Outputs to crates/turbomap-web/pkg.
RUN cd turbomap && ./crates/turbomap-web/build.sh --release

# ── Stage 2: React app (Vite) ───────────────────────────────────────────────
FROM node:22-bookworm-slim AS web
ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0
RUN corepack enable
WORKDIR /src/web
# Resolve deps first for layer caching. The `file:` dep resolves to
# ../turbomap/crates/turbomap-web/pkg, so the built pkg must be in place first.
COPY web/package.json ./
COPY --from=wasm /src/turbomap/crates/turbomap-web/pkg /src/turbomap/crates/turbomap-web/pkg
RUN pnpm install --no-frozen-lockfile
COPY web/ ./
RUN pnpm build

# ── Stage 3: static nginx runtime (unprivileged, :8080) ─────────────────────
FROM nginxinc/nginx-unprivileged:1.27-alpine AS runtime
COPY web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=web /src/web/dist /usr/share/nginx/html
EXPOSE 8080
