#!/bin/bash
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

FLUTTER_VERSION="3.41.2"
FLUTTER_DIR="/opt/flutter"

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

cd "$CLAUDE_PROJECT_DIR"
flutter pub get
