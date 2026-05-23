# Builds the `tileserver` binary plus the React admin SPA (apps/admin)
# into a single runtime image. ServeDir in the bin crate serves the
# SPA at /admin/app; admin JSON endpoints live at /admin/api/*.
#
# Build context expected at the repo root so we can COPY both
# apps/tileserver/ and apps/admin/. Override via the compose
# `context: ../../` and `dockerfile: apps/tileserver/Dockerfile`.

# ── Stage 1: build the admin SPA ──────────────────────────────────────
FROM node:22-bookworm-slim AS spa-builder
WORKDIR /build/admin
COPY apps/admin/package.json apps/admin/package-lock.json* ./
RUN npm ci || npm install
COPY apps/admin/. .
RUN npm run build

# ── Stage 2: plan + cook Cargo deps ──────────────────────────────────
FROM rust:1.90-bookworm AS chef
RUN cargo install cargo-chef --locked --version 0.1.68
WORKDIR /build

FROM chef AS planner
COPY apps/tileserver/. .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY apps/tileserver/. .
# SQLX_OFFLINE=true keeps the build hermetic — we don't use compile-time
# query checking yet (M1 query is a format!() string), so this flag is
# a safety net for when it's introduced.
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin tileserver

# ── Stage 3: runtime ─────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
# `raster2pgsql` (from `postgis`) and `psql` (from
# `postgresql-client`) are required by the `dtm-load` ingest job
# (apps/tileserver/crates/turbo-tiles-ingest/src/dtm_raster.rs).
# Skipping these means the container starts but every bulk-DEM
# import fails with "raster2pgsql: command not found" — caught only
# at runtime, not at image build time. ca-certificates is also
# needed by reqwest for the live Hoydedata + WFS endpoints.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        postgresql-client \
        postgis \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/tileserver /usr/local/bin/tileserver
COPY --from=spa-builder /build/admin/dist /var/lib/tileserver/admin/dist
ENV ADMIN_SPA_DIR=/var/lib/tileserver/admin/dist
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/tileserver"]
CMD ["serve"]
