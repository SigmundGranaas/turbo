#!/usr/bin/env bash
# The routing refactor gate.
#
# Runs the Sjunkhatten corpus on BOTH solver lanes and compares geometry
# hashes + DEM lookup counts against the committed baseline. This is the
# acceptance check for every structural step in the routing engine plan
# (docs/architecture/2026-07-routing-engine-implementation-plan.md §2).
#
# Why hashes and not timing: run-to-run wall clock varies 16-19% on a
# shared host, while the geometry hash and lookup count are exactly stable.
# Never gate a refactor on timing; measure timing separately.
#
#   ./tools/routing_gate.sh                 # check against the baseline
#   ./tools/routing_gate.sh --update        # accept current as the baseline
#
# Env:
#   TILESERVER_ARTIFACT_DIR   artifacts dir (default ~/.data/artifacts)
#
# Exit: 0 pass, 1 regress, 2 harness error.
set -uo pipefail
cd "$(dirname "$0")/.."

ART="${TILESERVER_ARTIFACT_DIR:-/home/user/turbo/.data/artifacts}"
CORPUS="tools/sjunkhatten-corpus.toml"
BASELINE="tools/sjunkhatten-baseline.json"
BIN="target/release/tileserver"
OUT="${TMPDIR:-/tmp}/routing-gate"
UPDATE=0
[[ "${1:-}" == "--update" ]] && UPDATE=1

[[ -x "$BIN" ]] || { echo "no $BIN — cargo build --release --bin tileserver"; exit 2; }
[[ -d "$ART" ]] || { echo "no artifacts at $ART"; exit 2; }
[[ -f "$CORPUS" ]] || { echo "no corpus at $CORPUS"; exit 2; }

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
