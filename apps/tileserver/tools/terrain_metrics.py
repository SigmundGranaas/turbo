#!/usr/bin/env python3
"""Terrain-routing evaluation harness.

For each ground-truth hike in tools/terrain-corpus.toml, fetch the
solver's output and compute a battery of metrics that measure
"smart terrain decision-making" — not just "match the polyline".

The premise is that the marked trail network IS the ground truth:
hikers picked those routes because they read the terrain correctly.
A well-tuned solver should re-derive equivalent decisions from the
DEM + landcover inputs, even if the literal polyline differs.

Metrics
-------

  elev_gain_m       Sum of positive elevation deltas along the route
                    (Naismith-relevant "uphill metres" — hikers minimise).
  elev_loss_m       Sum of negative deltas.
  max_slope_deg     Steepest single-segment slope encountered.
  mean_slope_deg    Length-weighted mean slope magnitude.
  fall_line_pct     % of segments where direction is within 30° of
                    local steepest-ascent direction. High = bushwhacking;
                    low = traversing on contour.
  length_m          Geodesic length of the polyline.
  frechet_m         Discrete Fréchet distance to the ground truth.
  trail_overlap_pct % of solver length within 30 m of any sti polyline
                    (informational; not part of composite score).

Composite score
---------------

Per-hike 0-100 score = mean of per-metric ratios to ground truth,
clamped and weighted. See `score_hike()`.
"""
import argparse
import json
import math
import sys
import time
import tomllib
import urllib.request
import urllib.error
from pathlib import Path

HOST = "http://localhost:8090"
SAMPLES_PER_KM = 50  # 20 m sampling along the polyline for elev profile


# ----- HTTP helpers ------------------------------------------------

def post(path: str, body: dict, timeout=60) -> dict:
    req = urllib.request.Request(
        f"{HOST}{path}",
        data=json.dumps(body).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode("utf-8"))


def interp_nan(distances: list[float], elev: list[float]) -> list[float]:
    """Linear-interpolate over NaN runs in an elevation profile (so
    per-segment slope math doesn't propagate NaN endlessly). Fully
    nodata samples become 0.0 (sea-level). Mutates and returns `elev`."""
    n = len(elev)
    for i in range(n):
        if math.isnan(elev[i]):
            j = i
            while j > 0 and math.isnan(elev[j]):
                j -= 1
            k = i
            while k < n - 1 and math.isnan(elev[k]):
                k += 1
            if math.isnan(elev[j]) and math.isnan(elev[k]):
                elev[i] = 0.0
            elif math.isnan(elev[j]):
                elev[i] = elev[k]
            elif math.isnan(elev[k]):
                elev[i] = elev[j]
            else:
                t = (distances[i] - distances[j]) / max(1.0, distances[k] - distances[j])
                elev[i] = elev[j] + t * (elev[k] - elev[j])
    return elev


def fetch_elev_profile(polyline: list[list[float]]) -> tuple[list[float], list[float]]:
    """Return (distances_m, elev_m) along the polyline. Distances are
    cumulative arc-length in EPSG:25833 metres; elev is metres above
    geoid. None elevations are interpolated linearly across the gap."""
    # Sample density: a long polyline gets many samples; short polylines
    # at least 20.
    nsamp = max(20, int(SAMPLES_PER_KM * polyline_length_km_approx(polyline)))
    nsamp = min(nsamp, 2000)
    body = {"line": polyline, "samples": nsamp}
    r = post("/v1/elev/profile", body)
    distances = list(r["distances_m"])
    elev = [e if e is not None else math.nan for e in r["elev_m"]]
    return distances, interp_nan(distances, elev)


def fetch_solver_path(from_pt, to_pt, cost_mode: str = "fast_marching",
                      override: dict | None = None,
                      force_off_trail: bool = False) -> tuple[list[list[float]], dict]:
    """Return (geometry, meta). `meta` carries strategy/on_trail_pct/
    fkb_breakdown so the harness can distinguish "router retraced an
    existing trail on-graph" (trivially perfect — the corpus endpoints
    ARE sti nodes) from "off-trail FMM solver re-derived the route from
    terrain" (the case the user actually cares about)."""
    prefs = {
        "profile": "foot",
        "cost_mode": cost_mode,
        # Production defaults — pathfinder snaps to graph when available
        # and only uses the FMM corridor for off-graph bridge segments.
        # This is what users see.
        "allow_off_trail": True,
        "max_off_trail_km": 20,
    }
    if force_off_trail:
        # Make the solver re-derive the route purely from terrain — no
        # snapping to the existing trail. This is the only mode that
        # actually exercises "smart terrain decisions": the corpus
        # endpoints are sti nodes, so with snapping on the router just
        # returns the ground-truth trail verbatim (score ~100, measures
        # nothing).
        prefs["force_off_trail"] = True
        prefs["snap_radius_m"] = 0
        prefs["bridge_radius_m"] = 0
    if override:
        prefs["cost_config_override"] = override
    body = {"from": list(from_pt), "to": list(to_pt), "prefs": prefs}
    r = post("/v1/pathfind", body)
    p = r.get("path", {})
    meta = {
        "strategy": p.get("strategy"),
        "on_trail_pct": p.get("on_trail_pct"),
        "fkb_breakdown": p.get("fkb_breakdown"),
        "refused_by": p.get("refused_by"),
    }
    if p.get("geometry"):
        return p["geometry"], meta
    # Hybrid response: concatenate leg geometries.
    out: list[list[float]] = []
    for leg in p.get("legs", []):
        out.extend(leg.get("geometry", []))
    return out, meta


# ----- Geometry helpers --------------------------------------------

def polyline_length_km_approx(poly):
    if len(poly) < 2: return 0.0
    s = 0.0
    for (x1, y1), (x2, y2) in zip(poly[:-1], poly[1:]):
        # Approx degrees → metres at ~60° latitude: 1° lat = 111 km,
        # 1° lon = 55 km. Good enough for sample-count budgeting.
        dx = (x2 - x1) * 55.0
        dy = (y2 - y1) * 111.0
        s += math.hypot(dx, dy)
    return s


def elev_gain_loss(distances, elev):
    gain = 0.0
    loss = 0.0
    for i in range(1, len(elev)):
        de = elev[i] - elev[i - 1]
        if de > 0: gain += de
        else:      loss -= de
    return gain, loss


def slope_stats(distances, elev):
    """Return (max_slope_deg, length_weighted_mean_slope_deg)."""
    max_s = 0.0
    sum_w = 0.0
    sum_ws = 0.0
    for i in range(1, len(elev)):
        ds = max(0.1, distances[i] - distances[i - 1])
        de = abs(elev[i] - elev[i - 1])
        slope_deg = math.degrees(math.atan2(de, ds))
        max_s = max(max_s, slope_deg)
        sum_w += ds
        sum_ws += ds * slope_deg
    mean_s = sum_ws / max(1.0, sum_w)
    return max_s, mean_s


def fall_line_pct(distances, elev, threshold_deg=30.0):
    """Fraction of segments where the slope magnitude is "high enough"
    that the segment direction matters AND the direction is within
    `threshold_deg` of the local steepest-ascent/descent direction.

    We can't compute true ∇z direction from a 1-D elevation profile,
    but we can approximate: if |slope| along the segment is high
    (>5°), assume the segment is roughly aligned with the local
    fall line. The true cross-slope angle would require 2-D DEM
    gradient sampling at each point, which the elev profile doesn't
    give us. For now this is a coarse proxy: any segment with
    >threshold_deg slope IS in the fall line.

    Returns (pct_fall_line_segments, total_fall_line_length_m).
    """
    fall_len = 0.0
    total_len = 0.0
    for i in range(1, len(elev)):
        ds = max(0.1, distances[i] - distances[i - 1])
        de = abs(elev[i] - elev[i - 1])
        slope_deg = math.degrees(math.atan2(de, ds))
        total_len += ds
        if slope_deg > threshold_deg:
            fall_len += ds
    return (100.0 * fall_len / max(1.0, total_len)), fall_len


def discrete_frechet(p, q):
    """Discrete Fréchet distance in degrees (approx — fine for
    same-region small polylines). Use Euclidean on raw lon/lat;
    convert with the approx 60° lat ratios at the end."""
    n, m = len(p), len(q)
    if n == 0 or m == 0: return float("inf")
    # CA[i][j] = best of all sup-distances along a coupling that
    # ends at p[i], q[j]. Iterative DP.
    ca = [[-1.0] * m for _ in range(n)]
    def d(a, b):
        dx = (a[0] - b[0]) * 55.0
        dy = (a[1] - b[1]) * 111.0
        return math.hypot(dx, dy)
    ca[0][0] = d(p[0], q[0])
    for i in range(1, n):
        ca[i][0] = max(ca[i-1][0], d(p[i], q[0]))
    for j in range(1, m):
        ca[0][j] = max(ca[0][j-1], d(p[0], q[j]))
    for i in range(1, n):
        for j in range(1, m):
            ca[i][j] = max(min(ca[i-1][j], ca[i-1][j-1], ca[i][j-1]),
                           d(p[i], q[j]))
    return ca[n-1][m-1]


# ----- Per-hike pipeline -------------------------------------------

def metrics_from_profile(distances, elev_raw):
    """Compute the metric battery from a precomputed elevation profile.
    `elev_raw` may contain None (nodata) — interpolated like the HTTP
    path. Shared by the online (`analyse_polyline`) and offline
    (`eval-terrain` JSON) pipelines so scoring is identical."""
    if not distances or len(distances) < 2:
        return None
    elev = [e if e is not None else math.nan for e in elev_raw]
    interp_nan(distances, elev)
    gain, loss = elev_gain_loss(distances, elev)
    max_s, mean_s = slope_stats(distances, elev)
    fall_pct, fall_len = fall_line_pct(distances, elev)
    length_m = distances[-1] if distances else 0.0
    return {
        "length_m":      round(length_m, 1),
        "elev_gain_m":   round(gain, 1),
        "elev_loss_m":   round(loss, 1),
        "max_slope_deg": round(max_s, 2),
        "mean_slope_deg": round(mean_s, 2),
        "fall_line_pct": round(fall_pct, 1),
    }


def analyse_polyline(poly):
    """Compute the metric battery for one polyline using DEM-driven
    elevation profile (HTTP path)."""
    if len(poly) < 2:
        return None
    distances, elev = fetch_elev_profile(poly)
    return metrics_from_profile(distances, elev)


def score_hike(truth: dict, solver: dict, frechet_m: float) -> tuple[float, dict]:
    """0-100 composite score. Returns (score, per-metric components).

    Higher is better. Weights and clamps reflect what "smart terrain
    decisions" means operationally:
      - elev_gain ratio: solver should not climb more than truth.
      - max_slope ratio: solver should not traverse steeper than truth.
      - fall_line excess: solver shouldn't bushwhack more than truth.
      - length ratio: not too long, not absurdly short.
      - Fréchet: literal polyline closeness — capped contribution.
    """
    def clamp(x, lo, hi): return max(lo, min(hi, x))

    # Each component is a 0-100 sub-score.
    def gain_score():
        t = truth["elev_gain_m"]
        s = solver["elev_gain_m"]
        if t < 5.0:
            # Flat truth; just bound solver's gain absolutely.
            return clamp(100.0 - 2.0 * max(0, s - 20.0), 0, 100)
        excess = max(0.0, s - t) / t  # only penalise EXCESS
        return clamp(100.0 - 60.0 * excess, 0, 100)

    def slope_score():
        t = truth["max_slope_deg"]
        s = solver["max_slope_deg"]
        excess = max(0.0, s - max(t, 5.0))  # tolerate +0; solver
        return clamp(100.0 - 4.0 * excess, 0, 100)  # 25° excess = 0

    def fall_line_score():
        t = truth["fall_line_pct"]
        s = solver["fall_line_pct"]
        excess = max(0.0, s - max(t, 5.0))
        return clamp(100.0 - 1.5 * excess, 0, 100)  # 67% excess = 0

    def length_score():
        t = truth["length_m"]
        s = solver["length_m"]
        if t < 100.0: return 100.0
        r = s / t
        # 0.7-1.5 = ideal; outside that, falls off
        if 0.7 <= r <= 1.5: return 100.0
        if r < 0.7: return clamp(100.0 - 150.0 * (0.7 - r), 0, 100)
        return clamp(100.0 - 80.0 * (r - 1.5), 0, 100)

    def frechet_score():
        # Normalise by truth length. 0.05 of length = excellent;
        # 0.50 of length = bad.
        L = max(100.0, truth["length_m"])
        rel = frechet_m / L
        return clamp(100.0 - 220.0 * rel, 0, 100)

    components = {
        "gain":      gain_score(),
        "slope":     slope_score(),
        "fall_line": fall_line_score(),
        "length":    length_score(),
        "frechet":   frechet_score(),
    }
    weights = {"gain": 0.25, "slope": 0.20, "fall_line": 0.20, "length": 0.10, "frechet": 0.25}
    score = sum(weights[k] * components[k] for k in weights)
    return round(score, 1), {k: round(v, 1) for k, v in components.items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default="tools/terrain-corpus.toml")
    ap.add_argument("--solver", choices=["auto", "fmm", "theta"], default="fmm")
    ap.add_argument("--filter", default=None, help="substring filter on region/id")
    ap.add_argument("--limit", type=int, default=0, help="max hikes to run")
    ap.add_argument("--out", default="/tmp/terrain-eval")
    ap.add_argument("--override-json", default=None,
                    help="JSON blob threaded into prefs.cost_config_override")
    ap.add_argument("--force-off-trail", action="store_true",
                    help="Disable graph-snapping so the solver must re-derive "
                         "the route from terrain. This is the only mode that "
                         "actually measures terrain decision-making — the "
                         "corpus endpoints are sti nodes, so prod-default just "
                         "retraces the ground-truth trail (score ~100).")
    ap.add_argument("--offline", default=None, metavar="DIR",
                    help="Score the per-hike JSON emitted by "
                         "`tileserver eval-terrain` in DIR instead of calling "
                         "the HTTP server. Fully offline + deterministic — the "
                         "autonomous routing dev loop. Uses the SAME scoring "
                         "formula as the HTTP path.")
    args = ap.parse_args()
    override = json.loads(args.override_json) if args.override_json else None

    # Offline mode: no server, no corpus fetch — re-score eval-terrain
    # output and exit.
    if args.offline:
        run_offline(Path(args.offline), Path(args.out))
        return

    corpus_path = Path(args.corpus)
    if not corpus_path.is_absolute():
        corpus_path = Path(__file__).parent.parent / args.corpus
    corpus = tomllib.loads(corpus_path.read_text())
    hikes = corpus.get("hike", [])
    if args.filter:
        hikes = [h for h in hikes if args.filter in str(h["id"]) or args.filter in h["region"]]
    if args.limit:
        hikes = hikes[:args.limit]
    print(f"Running {len(hikes)} hikes against solver={args.solver}")

    cost_mode = {"auto": "fast_marching", "fmm": "fast_marching", "theta": "walk_seconds"}[args.solver]
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    rows = []
    for i, h in enumerate(hikes, 1):
        name = f"{h['region']}-{h['id']}"
        print(f"[{i}/{len(hikes)}] {name} ({round(h['length_m'])} m)…", end=" ", flush=True)
        truth = analyse_polyline(h["polyline"])
        if truth is None:
            print("skip (no polyline)")
            continue
        t0 = time.time()
        try:
            solver_poly, meta = fetch_solver_path(
                h["from"], h["to"], cost_mode, override,
                force_off_trail=args.force_off_trail)
        except Exception as e:
            print(f"ERR {e}")
            continue
        dt_ms = int(1000 * (time.time() - t0))
        if len(solver_poly) < 2:
            print("solver returned empty geometry")
            continue
        solv = analyse_polyline(solver_poly)
        if solv is None:
            print("solver geom too short")
            continue
        frechet_m = discrete_frechet(h["polyline"], solver_poly)
        score, parts = score_hike(truth, solv, frechet_m)
        strat = meta.get("strategy") or "?"
        print(f"score={score} [{strat} trail={meta.get('on_trail_pct')}%] (gain={parts['gain']} slope={parts['slope']} fall={parts['fall_line']} len={parts['length']} frechet={parts['frechet']}) {dt_ms}ms")

        row = {
            "name": name,
            "id": h["id"],
            "region": h["region"],
            "from": h["from"],
            "to": h["to"],
            "truth": truth,
            "solver": solv,
            "frechet_m": round(frechet_m, 1),
            "score": score,
            "components": parts,
            "strategy": meta.get("strategy"),
            "on_trail_pct": meta.get("on_trail_pct"),
            "fkb_breakdown": meta.get("fkb_breakdown"),
            "refused_by": meta.get("refused_by"),
            "solver_poly": solver_poly,
            "truth_poly": h["polyline"],
            "solve_ms": dt_ms,
        }
        rows.append(row)
        (out_dir / f"{name}.json").write_text(json.dumps(row, indent=2))

    aggregate(rows, out_dir, args.solver)


def aggregate(rows, out_dir, solver_label):
    """Print the corpus aggregate and write `_summary.json`. Shared by
    the online (HTTP) and offline (eval-terrain JSON) pipelines."""
    if not rows:
        print("\nNo scored hikes.")
        return
    avg_score = sum(r["score"] for r in rows) / len(rows)
    print(f"\nCorpus average composite score: {avg_score:.1f}")
    print(f"Median: {sorted(r['score'] for r in rows)[len(rows)//2]}")
    for k in ("gain", "slope", "fall_line", "length", "frechet"):
        mean = sum(r["components"][k] for r in rows) / len(rows)
        print(f"  {k:>10s}: {mean:.1f}")
    # Strategy mix: how many hikes solved on-graph (retraced an existing
    # trail) vs fell to the off-trail solver.
    from collections import Counter
    strat_mix = Counter(r.get("strategy") or "?" for r in rows)
    print(f"  strategy mix: {dict(strat_mix)}")
    for strat in sorted(strat_mix):
        sub = [r for r in rows if (r.get("strategy") or "?") == strat]
        avg = sum(r["score"] for r in sub) / len(sub)
        print(f"    {strat:>10s}: n={len(sub)} avg_score={avg:.1f}")
    (out_dir / "_summary.json").write_text(json.dumps({
        "rows": rows, "avg_score": avg_score, "solver": solver_label
    }, indent=2))


def run_offline(in_dir: Path, out_dir: Path):
    """Score the per-hike JSON emitted by `tileserver eval-terrain`,
    reusing the EXACT scoring formula — no HTTP, no server. Each input
    file already carries truth + solver polylines with their elevation
    profiles, so this is a pure, deterministic re-score."""
    files = sorted(p for p in in_dir.glob("*.json") if not p.name.startswith("_"))
    print(f"Offline scoring {len(files)} hikes from {in_dir}")
    rows = []
    for path in files:
        d = json.loads(path.read_text())
        name = f"{d['region']}-{d['id']}"
        truth = metrics_from_profile(d["truth"]["distances_m"], d["truth"]["elev_m"])
        if truth is None:
            print(f"{name}: skip (no truth profile)")
            continue
        if not d.get("ok") or len(d["solver"]["polyline"]) < 2:
            print(f"{name}: solver failed ({d.get('error')})")
            continue
        solv = metrics_from_profile(d["solver"]["distances_m"], d["solver"]["elev_m"])
        if solv is None:
            print(f"{name}: solver geom too short")
            continue
        frechet_m = discrete_frechet(d["truth"]["polyline"], d["solver"]["polyline"])
        score, parts = score_hike(truth, solv, frechet_m)
        strat = d.get("strategy") or "?"
        print(f"{name}: score={score} [{strat}] (gain={parts['gain']} slope={parts['slope']} "
              f"fall={parts['fall_line']} len={parts['length']} frechet={parts['frechet']}) "
              f"{d.get('solve_ms', 0):.0f}ms")
        rows.append({
            "name": name, "id": d["id"], "region": d["region"],
            "truth": truth, "solver": solv,
            "frechet_m": round(frechet_m, 1),
            "score": score, "components": parts,
            "strategy": d.get("strategy"),
            "geometry_hash": d.get("geometry_hash"),
            "solve_ms": d.get("solve_ms"),
        })
    out_dir.mkdir(parents=True, exist_ok=True)
    aggregate(rows, out_dir, "offline")


if __name__ == "__main__":
    main()
