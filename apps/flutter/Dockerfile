# Multi-stage build for the Turbo Flutter web app.
# Build context is apps/flutter (no path-deps outside it).
#
#   builder  -> flutter build web --release
#   runtime  -> nginx-unprivileged serving the static bundle on :8080
#
# API base URL defaults to https://kart-api.sandring.no in
# lib/core/config/env_config.dart; override at build time with
#   --build-arg API_BASE_URL=https://staging-api.example.com
FROM ghcr.io/cirruslabs/flutter:3.41.2 AS builder

ARG API_BASE_URL=""
WORKDIR /app

# Resolve dependencies first for better layer caching.
COPY pubspec.yaml pubspec.lock ./
COPY tools/ tools/
RUN flutter pub get

# Build the web bundle.
COPY . .
RUN flutter build web --release --dart-define=API_BASE_URL="${API_BASE_URL}"

FROM nginxinc/nginx-unprivileged:1.27-alpine AS runtime
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/build/web /usr/share/nginx/html
EXPOSE 8080
