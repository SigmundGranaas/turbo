#!/bin/bash
# Session-start hook for Claude Code on the web.
#
# Installs .NET 10 SDK so `dotnet build` / `dotnet test` work, starts the
# Docker daemon so Testcontainers-backed behaviour tests can spin up
# Postgres + NATS, then warms `dotnet restore` so the first build is fast.
#
# Idempotent: every install step probes for the artefact first, so the
# hook is safe to re-run.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

# ---------------------------------------------------------------------------
# 1. .NET 10 SDK via Microsoft's apt repo.
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
# 2. Docker daemon for Testcontainers.
# ---------------------------------------------------------------------------
if command -v dockerd >/dev/null 2>&1; then
  if ! docker info >/dev/null 2>&1; then
    # Start dockerd in the background; the daemon logs to /tmp so they're
    # available if a behaviour test fails for an infra reason.
    nohup dockerd >/tmp/dockerd.log 2>&1 &
    # Wait for the socket so the next `docker` command doesn't race.
    for _ in $(seq 1 30); do
      docker info >/dev/null 2>&1 && break
      sleep 1
    done
  fi
fi

# ---------------------------------------------------------------------------
# 3. Restore NuGet packages.
# ---------------------------------------------------------------------------
cd "$CLAUDE_PROJECT_DIR"
dotnet restore Turboapi.sln
