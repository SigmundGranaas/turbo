#!/usr/bin/env bash
# The routing refactor gate.
#
# Runs a corpus on BOTH solver lanes and compares geometry hashes + DEM
# lookup counts against a committed baseline. This is the acceptance
# check for every structural step in the routing engine plan
# (docs/architecture/2026-07-routing-engine-implementation-plan.md §2).
#
# Why hashes and not timing: run-to-run wall clock varies 16-19% on a
# shared host, while the geometry hash and lookup count are exactly
# stable. Never gate a refactor on timing; measure timing separately.
#
# ── Two profiles ──────────────────────────────────────────────────────
#
#   ci    (default)  25 hikes against tools/ci-pack — 4.6 MB, committed,
#                    so this runs anywhere the repo is checked out.
#   full             90 hikes against the full 209 MB Sjunkhatten
#                    artifacts, which have to be provisioned by hand.
#
# The two baselines are NOT comparable, and nothing here invites you to
# compare them. `ci-pack` is a slice: edges crossing its boundary are
# dropped (changing trail-proximity bonuses) and defect D8 means
# dropping a DEM tile can change `sample` where tiles overlap. Each
# profile is a self-consistent regression gate against its own history.
#
# Run `full` before landing anything that moves geometry on purpose
# (Phase E calibration); `ci` is enough for structural work, which is
# what it is checked in for.
#
#   ./tools/routing_gate.sh                  # ci profile, check
#   ./tools/routing_gate.sh --update         # ci profile, rebaseline
#   ./tools/routing_gate.sh full             # full profile, check
#   ./tools/routing_gate.sh full --update    # full profile, rebaseline
#
# Env:
#   TILESERVER_ARTIFACT_DIR   overrides the artifacts dir (full profile)
#
# Exit: 0 pass, 1 regress, 2 harness error.
set -uo pipefail
cd "$(dirname "$0")/.."

PROFILE=ci
UPDATE=0
for arg in "$@"; do
  case "$arg" in
    ci|full) PROFILE="$arg" ;;
    --update) UPDATE=1 ;;
    *) echo "usage: $0 [ci|full] [--update]"; exit 2 ;;
  esac
done

case "$PROFILE" in
  ci)
    ART="tools/ci-pack"
    CORPUS="tools/sjunkhatten-ci-corpus.toml"
    BASELINE="tools/sjunkhatten-ci-baseline.json"
    ;;
  full)
    ART="${TILESERVER_ARTIFACT_DIR:-/home/user/turbo/.data/artifacts}"
    CORPUS="tools/sjunkhatten-corpus.toml"
    BASELINE="tools/sjunkhatten-baseline.json"
    ;;
esac

BIN="target/release/tileserver"
OUT="${TMPDIR:-/tmp}/routing-gate-$PROFILE"

[[ -x "$BIN" ]] || { echo "no $BIN — cargo build --release -p turbo-tiles-bin --bin tileserver"; exit 2; }
[[ -d "$ART" ]] || {
  echo "no artifacts at $ART"
  [[ "$PROFILE" == full ]] && echo "  (the full profile needs the 209 MB Sjunkhatten set; try: $0 ci)"
  exit 2
}
[[ -f "$CORPUS" ]] || { echo "no corpus at $CORPUS"; exit 2; }

echo "profile: $PROFILE   artifacts: $ART   corpus: $CORPUS"
rm -rf "$OUT"; mkdir -p "$OUT"
declare -A GOT
for lane in off-trail unified; do
  TILESERVER_ARTIFACT_DIR="$ART" timeout 2400 "$BIN" eval-terrain \
      --corpus="$CORPUS" --artifacts-dir="$ART" --mode="$lane" \
      --out="$OUT/$lane" > "$OUT/$lane.log" 2>&1 \
    || { echo "eval-terrain failed on $lane; see $OUT/$lane.log"; exit 2; }
  GOT[$lane]=$(python3 -c "
import json;d=json.load(open('$OUT/$lane/_summary.json'))
print(json.dumps({'hash':d['corpus_geometry_hash'],'lookups':d['dem_cache_lookups'],'ok':d['ok'],'total':d['total']}))")
done

if [[ $UPDATE -eq 1 ]]; then
  python3 - "$BASELINE" "${GOT[off-trail]}" "${GOT[unified]}" <<'PY'
import json, sys
out = {"off-trail": json.loads(sys.argv[2]), "unified": json.loads(sys.argv[3])}
open(sys.argv[1], "w").write(json.dumps(out, indent=2) + "\n")
print(f"baseline updated: {sys.argv[1]}")
for k, v in out.items():
    print(f"  {k:<10} {v['hash']}  lookups={v['lookups']}  ok={v['ok']}/{v['total']}")
PY
  exit 0
fi

[[ -f "$BASELINE" ]] || { echo "no baseline — run with --update first"; exit 2; }
python3 - "$BASELINE" "${GOT[off-trail]}" "${GOT[unified]}" <<'PY'
import json, sys
base = json.load(open(sys.argv[1]))
got = {"off-trail": json.loads(sys.argv[2]), "unified": json.loads(sys.argv[3])}
bad = False
for lane in ("off-trail", "unified"):
    b, g = base[lane], got[lane]
    for field in ("hash", "lookups", "ok"):
        if b[field] != g[field]:
            print(f"REGRESS {lane}.{field}: {b[field]} -> {g[field]}")
            bad = True
    if not bad:
        print(f"ok      {lane:<10} {g['hash']}  lookups={g['lookups']}  ok={g['ok']}/{g['total']}")
sys.exit(1 if bad else 0)
PY
