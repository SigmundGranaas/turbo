#!/usr/bin/env bash
#
# Owns the whole Turbo iOS dev + validation stack in one command:
#   1. (re)generate the Xcode project from project.yml
#   2. run the SwiftPM unit/integration suite on the host
#   3. boot a simulator and run the end-to-end XCUITest suite
#
# Usage:  apps/ios/scripts/test.sh [--unit-only] [--e2e-only]
# Exit code is non-zero if any stage fails — safe for CI / pre-push hooks.

set -euo pipefail

cd "$(dirname "$0")/.."   # apps/ios

UNIT=1
E2E=1
case "${1:-}" in
  --unit-only) E2E=0 ;;
  --e2e-only)  UNIT=0 ;;
esac

bold() { printf "\n\033[1m== %s ==\033[0m\n" "$1"; }

bold "1/3 · Generating Xcode project (xcodegen)"
if ! command -v xcodegen >/dev/null 2>&1; then
  echo "xcodegen not found — install with: brew install xcodegen" >&2
  exit 1
fi
xcodegen generate

if [[ "$UNIT" == 1 ]]; then
  bold "2/3 · Unit + integration tests (swift test)"
  # The swift-testing helper occasionally crashes (SIGSEGV) on spin-up with 0
  # tests run; retry a couple of times before treating it as a real failure.
  attempt=1
  until swift test; do
    if [[ $attempt -ge 3 ]]; then echo "swift test failed after $attempt attempts" >&2; exit 1; fi
    echo "swift test crashed on attempt $attempt — retrying…" >&2
    attempt=$((attempt + 1))
  done
else
  echo "(skipping unit tests)"
fi

if [[ "$E2E" == 1 ]]; then
  bold "3/3 · End-to-end UI tests (xcodebuild test)"

  # Pick an available iPhone simulator by name; let xcodebuild boot it.
  DEVICE=$(xcrun simctl list devices available \
    | grep -oE 'iPhone [0-9]+( Pro Max| Pro| Plus)?' | sort -rV | head -1)
  if [[ -z "$DEVICE" ]]; then
    echo "No iPhone simulator available" >&2
    exit 1
  fi
  echo "Simulator: $DEVICE"

  # -uitest is set by the test code's launchArguments; here we just run the suite.
  set -o pipefail
  RESULT_BUNDLE="$(mktemp -d)/Turbo.xcresult"
  xcodebuild test \
    -project Turbo.xcodeproj \
    -scheme Turbo \
    -destination "platform=iOS Simulator,name=$DEVICE" \
    -resultBundlePath "$RESULT_BUNDLE" \
    -quiet
  echo "Result bundle: $RESULT_BUNDLE"
else
  echo "(skipping E2E tests)"
fi

bold "ALL CHECKS PASSED"
