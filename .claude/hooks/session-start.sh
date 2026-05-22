#!/bin/bash
# Session-start hook for Claude Code on the web. Sets up both the Flutter
# app (apps/flutter) and the .NET API (apps/api) so either side is ready
# to build and test on first invocation.
#
# Idempotent: every install step probes for the artefact first, so the
# hook is safe to re-run.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

FLUTTER_VERSION="3.41.2"
FLUTTER_DIR="/opt/flutter"

# ---------------------------------------------------------------------------
# 1. SQLite (used by Flutter tests) and Flutter SDK.
# ---------------------------------------------------------------------------
if ! dpkg -s sqlite3 libsqlite3-dev >/dev/null 2>&1; then
  apt-get update -qq || true
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends sqlite3 libsqlite3-dev
fi

if [ ! -x "$FLUTTER_DIR/bin/flutter" ]; then
  ARCHIVE="flutter_linux_${FLUTTER_VERSION}-stable.tar.xz"
  URL="https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/${ARCHIVE}"
  TMP_DIR="$(mktemp -d)"
  curl -fsSL "$URL" -o "$TMP_DIR/$ARCHIVE"
  mkdir -p "$(dirname "$FLUTTER_DIR")"
  tar -xf "$TMP_DIR/$ARCHIVE" -C "$(dirname "$FLUTTER_DIR")"
  rm -rf "$TMP_DIR"
fi

git config --global --add safe.directory "$FLUTTER_DIR"

export PATH="$FLUTTER_DIR/bin:$PATH"
echo "export PATH=\"$FLUTTER_DIR/bin:\$PATH\"" >> "$CLAUDE_ENV_FILE"

flutter config --no-analytics >/dev/null
flutter precache --universal --linux --web --no-android --no-ios --no-macos --no-windows

if [ -f "$CLAUDE_PROJECT_DIR/apps/flutter/pubspec.yaml" ]; then
  (cd "$CLAUDE_PROJECT_DIR/apps/flutter" && flutter pub get)
fi

# ---------------------------------------------------------------------------
# 2. .NET 10 SDK via Microsoft's apt repo.
# ---------------------------------------------------------------------------
if ! command -v dotnet >/dev/null 2>&1; then
  if ! dpkg -s packages-microsoft-prod >/dev/null 2>&1; then
    TMP_DEB="$(mktemp --suffix=.deb)"
    curl -fsSL https://packages.microsoft.com/config/ubuntu/24.04/packages-microsoft-prod.deb -o "$TMP_DEB"
    dpkg -i "$TMP_DEB"
    rm -f "$TMP_DEB"
  fi
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends dotnet-sdk-10.0
fi

# ---------------------------------------------------------------------------
# 3. Docker daemon for Testcontainers (api behaviour tests).
# ---------------------------------------------------------------------------
if command -v dockerd >/dev/null 2>&1; then
  if ! docker info >/dev/null 2>&1; then
    nohup dockerd >/tmp/dockerd.log 2>&1 &
    for _ in $(seq 1 30); do
      docker info >/dev/null 2>&1 && break
      sleep 1
    done
  fi
fi

# ---------------------------------------------------------------------------
# 4. Restore NuGet packages.
# ---------------------------------------------------------------------------
if [ -f "$CLAUDE_PROJECT_DIR/apps/api/Turboapi.sln" ]; then
  (cd "$CLAUDE_PROJECT_DIR/apps/api" && dotnet restore Turboapi.sln)
fi
