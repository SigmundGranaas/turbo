#!/bin/bash
# Session-start hook for Claude Code on the web. Sets up the .NET API
# (apps/api) so it is ready to build and test on first invocation.
# (The Flutter app was removed in P5.3 — web + Android are the product.)
#
# Idempotent: every install step probes for the artefact first, so the
# hook is safe to re-run.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
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
