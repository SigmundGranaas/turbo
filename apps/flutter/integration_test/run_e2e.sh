#!/usr/bin/env bash
# Run the Flutter integration test suite against a local docker compose
# stack. Waits for the gateway's /healthz to return 200 before invoking
# `flutter test`, so a missing stack fails fast with a clear message
# instead of looking like an auth or routing bug.
#
# Usage (from apps/flutter/):
#   integration_test/run_e2e.sh                                # full suite
#   integration_test/run_e2e.sh path/to/one_test.dart          # one file
#
# Env overrides:
#   API_BASE_URL   default: http://localhost:8080
#   E2E_DEVICE     default: macos
#   E2E_WAIT_SECS  default: 30

set -euo pipefail

API_BASE_URL="${API_BASE_URL:-http://localhost:8080}"
E2E_DEVICE="${E2E_DEVICE:-macos}"
E2E_WAIT_SECS="${E2E_WAIT_SECS:-30}"

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
echo "Backend healthy. Running E2E suite on -d $E2E_DEVICE..."

# integration_test on desktop only supports one app instance per
# `flutter test` invocation. Running the whole `integration_test`
# directory fails the second file with "Unable to start the app on
# the device". So invoke each test file separately and aggregate.
if [[ $# -gt 0 ]]; then
  targets=("$@")
else
  targets=()
  while IFS= read -r f; do
    targets+=("$f")
  done < <(find integration_test -maxdepth 1 -name "*_test.dart" | sort)
fi

failed=()
for t in "${targets[@]}"; do
  echo
  echo "===== $t ====="
  if ! flutter test "$t" \
      -d "$E2E_DEVICE" \
      --dart-define=API_BASE_URL="$API_BASE_URL"; then
    failed+=("$t")
  fi
done

echo
if (( ${#failed[@]} == 0 )); then
  echo "All E2E test files passed."
  exit 0
fi
echo "FAILED:" >&2
printf '  - %s\n' "${failed[@]}" >&2
exit 1
