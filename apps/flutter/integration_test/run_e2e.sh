#!/usr/bin/env bash
# Run the Patrol-driven E2E suite against a local docker compose stack
# on the iOS Simulator. Waits for the gateway's /healthz before kicking
# off the test build so a missing stack fails fast with a clear
# message instead of looking like an auth or UI bug.
#
# Usage (from apps/flutter/):
#   integration_test/run_e2e.sh                                # full suite
#   integration_test/run_e2e.sh integration_test/<file>.dart   # one file
#
# Env overrides:
#   API_BASE_URL   default: http://localhost:8080
#   E2E_DEVICE     default: D3A062B4-2AB3-47C0-AE42-1CFA0FECE11A (iPhone 17 sim).
#                  Override with another sim UDID or `xcrun simctl list`.
#   E2E_WAIT_SECS  default: 30

set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://localhost:8080}"
E2E_DEVICE="${E2E_DEVICE:-D3A062B4-2AB3-47C0-AE42-1CFA0FECE11A}"
E2E_WAIT_SECS="${E2E_WAIT_SECS:-30}"
TARGET="${1:-integration_test/user_journeys_test.dart}"

export PATH="$PATH:$HOME/.pub-cache/bin"

if ! command -v patrol >/dev/null 2>&1; then
  echo "patrol CLI not on PATH. Install with:" >&2
  echo "  dart pub global activate patrol_cli" >&2
  exit 1
fi

echo "Waiting for backend at $API_BASE_URL/healthz (up to ${E2E_WAIT_SECS}s)..."
deadline=$(( $(date +%s) + E2E_WAIT_SECS ))
until curl -fsS -o /dev/null "$API_BASE_URL/healthz"; do
  if (( $(date +%s) >= deadline )); then
    echo "Backend never became healthy. Start it with:" >&2
    echo "  docker compose -f infra/compose/compose.yaml \\" >&2
    echo "                 -f infra/compose/compose.services.yaml up -d" >&2
    exit 1
  fi
  sleep 1
done

# Make sure the simulator is booted (patrol won't boot it for us).
xcrun simctl boot "$E2E_DEVICE" >/dev/null 2>&1 || true

echo "Backend healthy. Running Patrol suite on $E2E_DEVICE..."

exec patrol test \
  --target "$TARGET" \
  -d "$E2E_DEVICE" \
  --dart-define=API_BASE_URL="$API_BASE_URL"
