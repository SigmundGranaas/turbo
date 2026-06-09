#!/usr/bin/env python3
"""Autonomous routing development loop — ONE command, no server, no maps.

    python3 tools/routing_loop.py                # measure vs committed baseline → PASS/REGRESS
    python3 tools/routing_loop.py --update-baseline   # snapshot current as the new baseline
    python3 tools/routing_loop.py --no-build     # skip cargo build (use existing binary)

It chains the in-process headless evaluator and the offline scorer:

  1. cargo build --release -p turbo-tiles-bin
  2. tileserver eval-terrain --check-determinism   (solve corpus in-process)
  3. terrain_metrics.py --offline                  (score with the trusted formula)
  4. diff vs tools/routing-baseline.json           (quality / latency / determinism / geometry)

then prints a verdict across four axes. This is what lets an agent tweak
the routing engine and know — objectively, with zero human judgement —
whether a change helped, hurt, or moved any route. Solves are
deterministic, so the per-route geometry hash is an exact change-detector.

Exit code 0 = PASS, 1 = REGRESS, 2 = harness error.
"""
import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent          # apps/tileserver/tools
TILESERVER_DIR = HERE.parent                     # apps/tileserver
BASELINE_PATH = HERE / "routing-baseline.json"

# Regression thresholds (tuned to sit above run-to-run float noise,
# which is ~0 here since solves are deterministic on one machine).
QUALITY_AVG_DROP = 0.5      # corpus avg score drop that counts as a regress
QUALITY_HIKE_DROP = 3.0     # per-hike score drop worth flagging
LATENCY_P95_FACTOR = 1.25   # p95 regress if > baseline*factor + pad
LATENCY_P95_PAD_MS = 50.0
MEMORY_FACTOR = 1.20        # peak RSS regress if > baseline*factor + pad
MEMORY_PAD_MB = 64.0


def run(cmd, **kw):
    print(f"$ {' '.join(str(c) for c in cmd)}", flush=True)
    return subprocess.run(cmd, cwd=TILESERVER_DIR, check=True, **kw)


def collect(eval_dir: Path, score_dir: Path) -> dict:
    """Merge the eval summary (latency/determinism/geometry) and the
    offline score summary (quality) into one snapshot dict."""
    ev = json.loads((eval_dir / "_summary.json").read_text())
    # Per-hike geometry hashes live in the per-hike eval files.
    per_hash = {}
    for p in eval_dir.glob("*.json"):
        if p.name.startswith("_"):
            continue
        d = json.loads(p.read_text())
        per_hash[f"{d['region']}-{d['id']}"] = d.get("geometry_hash")

    sc = json.loads((score_dir / "_summary.json").read_text())
    per_score = {r["name"]: r["score"] for r in sc.get("rows", [])}

    return {
        "corpus": ev.get("corpus"),
        "quality": {"avg_score": sc.get("avg_score"), "per_hike": per_score},
        "latency": {
            "mean_ms": ev.get("solve_ms_mean"),
            "p50_ms": ev.get("solve_ms_p50"),
            "p95_ms": ev.get("solve_ms_p95"),
            "max_ms": ev.get("solve_ms_max"),
        },
        "memory": {"peak_rss_mb": ev.get("peak_rss_mb")},
        "work": {"dem_cache_lookups": ev.get("dem_cache_lookups")},
        "geometry": {
            "corpus_hash": ev.get("corpus_geometry_hash"),
            "per_hike": per_hash,
        },
        "determinism_ok": ev.get("determinism_ok"),
        "counts": {"total": ev.get("total"), "ok": ev.get("ok"), "failed": ev.get("failed")},
    }


def verdict(cur: dict, base: dict) -> bool:
    """Print a four-axis PASS/REGRESS report. Returns True if PASS."""
    ok = True
    print("\n" + "=" * 64)
    print("ROUTING LOOP VERDICT")
    print("=" * 64)

    # --- determinism (hard gate) ---
    det = cur.get("determinism_ok")
    if det is False:
        print("DETERMINISM  REGRESS  solves are nondeterministic this run")
        ok = False
    elif det is True:
        print("DETERMINISM  ok       identical geometry across two passes")
    else:
        print("DETERMINISM  -        not checked")

    # --- solve success ---
    failed = cur["counts"]["failed"]
    base_failed = base["counts"]["failed"] if base else None
    if base is not None and failed > base_failed:
        print(f"SOLVES       REGRESS  {failed} failed (baseline {base_failed})")
        ok = False
    else:
        print(f"SOLVES       ok       {cur['counts']['ok']}/{cur['counts']['total']} ok")

    if base is None:
        print("QUALITY      -        no baseline (use --update-baseline)")
        print("LATENCY      -        no baseline")
        print("GEOMETRY     -        no baseline")
        print("=" * 64)
        return ok

    # --- quality ---
    ca, ba = cur["quality"]["avg_score"], base["quality"]["avg_score"]
    delta = ca - ba
    regressed = [
        (n, base["quality"]["per_hike"][n], s)
        for n, s in cur["quality"]["per_hike"].items()
        if n in base["quality"]["per_hike"]
        and base["quality"]["per_hike"][n] - s >= QUALITY_HIKE_DROP
    ]
    if delta <= -QUALITY_AVG_DROP:
        print(f"QUALITY      REGRESS  avg {ba:.1f} -> {ca:.1f} ({delta:+.1f})")
        ok = False
    else:
        print(f"QUALITY      ok       avg {ba:.1f} -> {ca:.1f} ({delta:+.1f})")
    for n, b, s in sorted(regressed, key=lambda x: x[2] - x[1])[:8]:
        print(f"               - {n}: {b:.1f} -> {s:.1f} ({s - b:+.1f})")

    # --- latency ---
    cp, bp = cur["latency"]["p95_ms"], base["latency"]["p95_ms"]
    limit = bp * LATENCY_P95_FACTOR + LATENCY_P95_PAD_MS
    if cp > limit:
        print(f"LATENCY      REGRESS  p95 {bp:.0f}ms -> {cp:.0f}ms (limit {limit:.0f}ms)")
        ok = False
    else:
        print(f"LATENCY      ok       p95 {bp:.0f}ms -> {cp:.0f}ms")

    # --- memory ---
    cm = cur["memory"]["peak_rss_mb"]
    bm = base["memory"]["peak_rss_mb"]
    if cm is None or bm is None:
        print("MEMORY       -        not recorded in baseline")
    else:
        limit = bm * MEMORY_FACTOR + MEMORY_PAD_MB
        if cm > limit:
            print(f"MEMORY       REGRESS  peak RSS {bm:.0f}MiB -> {cm:.0f}MiB (limit {limit:.0f}MiB)")
            ok = False
        else:
            print(f"MEMORY       ok       peak RSS {bm:.0f}MiB -> {cm:.0f}MiB")

    # --- DEM work (deterministic; the noise-free perf signal) ---
    cw = cur["work"]["dem_cache_lookups"]
    bw = base["work"]["dem_cache_lookups"] if base.get("work") else None
    if cw is None or bw is None:
        print("DEM WORK     -        not recorded in baseline")
    elif cw > bw:
        # More DEM work than baseline is a real regression (deterministic).
        print(f"DEM WORK     REGRESS  cache lookups {bw:,} -> {cw:,} ({cw - bw:+,})")
        ok = False
    elif cw < bw:
        pct = 100.0 * (bw - cw) / bw if bw else 0.0
        print(f"DEM WORK     improved  cache lookups {bw:,} -> {cw:,} (-{pct:.0f}%)")
    else:
        print(f"DEM WORK     ok       cache lookups {cw:,} (unchanged)")

    # --- geometry drift (informational unless quality also moved) ---
    if cur["geometry"]["corpus_hash"] == base["geometry"]["corpus_hash"]:
        print("GEOMETRY     identical  no route changed")
    else:
        moved = [
            n for n, h in cur["geometry"]["per_hike"].items()
            if base["geometry"]["per_hike"].get(n) != h
        ]
        print(f"GEOMETRY     changed   {len(moved)} route(s) moved: "
              f"{', '.join(sorted(moved)[:10])}{' …' if len(moved) > 10 else ''}")
    print("=" * 64)
    return ok


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--artifacts-dir", default=None,
                    help="Routing artifacts dir (default: $TILESERVER_ARTIFACT_DIR or ~/turbo-artifacts)")
    ap.add_argument("--no-build", action="store_true", help="Skip cargo build")
    ap.add_argument("--update-baseline", action="store_true",
                    help="Write the current run as the new committed baseline and exit PASS")
    ap.add_argument("--filter", default=None, help="Substring filter on region/id")
    ap.add_argument("--limit", type=int, default=None, help="Max hikes")
    args = ap.parse_args()

    artifacts = args.artifacts_dir
    if not artifacts:
        import os
        artifacts = os.environ.get("TILESERVER_ARTIFACT_DIR") or str(Path.home() / "turbo-artifacts")

    if not args.no_build:
        run(["cargo", "build", "--release", "-p", "turbo-tiles-bin"])
    binary = TILESERVER_DIR / "target" / "release" / "tileserver"
    if not binary.exists():
        print(f"error: {binary} not found (drop --no-build)", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="routing-loop-") as tmp:
        eval_dir = Path(tmp) / "eval"
        score_dir = Path(tmp) / "score"
        eval_cmd = [binary, "eval-terrain", "--artifacts-dir", artifacts,
                    "--out", str(eval_dir), "--check-determinism"]
        if args.filter:
            eval_cmd += ["--filter", args.filter]
        if args.limit:
            eval_cmd += ["--limit", str(args.limit)]
        run(eval_cmd)
        run([sys.executable, str(HERE / "terrain_metrics.py"),
             "--offline", str(eval_dir), "--out", str(score_dir)])

        cur = collect(eval_dir, score_dir)

        if args.update_baseline:
            BASELINE_PATH.write_text(json.dumps(cur, indent=2, sort_keys=True))
            print(f"\nwrote baseline {BASELINE_PATH} "
                  f"(avg_score={cur['quality']['avg_score']:.1f}, "
                  f"p95={cur['latency']['p95_ms']:.0f}ms, "
                  f"corpus_hash={cur['geometry']['corpus_hash']})")
            return 0

        base = json.loads(BASELINE_PATH.read_text()) if BASELINE_PATH.exists() else None
        passed = verdict(cur, base)
        return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
