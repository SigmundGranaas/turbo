#!/usr/bin/env bash
# Device capture for AAA-water verification (see
# docs/architecture/2026-06-aaa-water-implementation-plan.md §0).
#
# Captures the CURRENT on-device map view + the latest per-frame trace metrics
# (fps / gpu_ms / memory / pending) so each phase's look AND budget are recorded.
# Navigate the device to a test view (V1 fjord-close … V5 shoreline), then:
#
#   tools/water_shots.sh <phase> <view>
#   e.g.  tools/water_shots.sh p1-gerstner V1-fjord-close
#
# Deterministic camera positioning (a debug `water_view` intent) lands with P1
# so views become fully repeatable; until then navigate by hand first.
set -euo pipefail

ADB="${ADB:-$HOME/Library/Android/sdk/platform-tools/adb}"
PKG=com.sigmundgranaas.turbo.expressive
PHASE="${1:-adhoc}"
VIEW="${2:-view}"
OUT="apps/turbomap/target/water-shots/$PHASE"
mkdir -p "$OUT"

# Keep the screen on so the wgpu surface keeps rendering during capture.
$ADB shell svc power stayon true >/dev/null 2>&1 || true
$ADB shell settings put system screen_off_timeout 1800000 >/dev/null 2>&1 || true
$ADB shell input keyevent KEYCODE_WAKEUP >/dev/null 2>&1 || true
sleep 1

# Screenshot.
$ADB exec-out screencap -p > "$OUT/$VIEW.png"
bytes=$(wc -c < "$OUT/$VIEW.png" | tr -d ' ')

# Latest trace metrics (the surface logs a PERF line + per-frame TurbomapTrace).
metrics=$($ADB logcat -d -v brief 2>/dev/null | grep -iE "PERF|TurbomapTrace" | tail -3 || true)
{
  echo "=== $PHASE / $VIEW @ $(date '+%F %T') ==="
  echo "shot: $OUT/$VIEW.png (${bytes} bytes)"
  echo "$metrics"
  echo
} >> "$OUT/metrics.txt"

echo "captured $OUT/$VIEW.png (${bytes} bytes)"
echo "$metrics" | tail -1
