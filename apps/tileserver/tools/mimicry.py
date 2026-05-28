#!/usr/bin/env python3
"""
Trail-mimicry harness for the off-trail pathfinder.

For each scenario in trail-mimicry.toml we run two solves:
  1. baseline — normal pathfind (uses graph + snap). This is the
     "real trail" reference.
  2. offgrid  — same endpoints with snap_radius_m=0 and
     bridge_radius_m=0, which forces the off_trail strategy.

We then compute three metrics:
  - mean_dev_m      mean of nearest-distance from each offgrid
                    vertex to the baseline polyline
  - max_dev_m       max nearest-distance
  - length_ratio    offgrid_length_m / baseline_length_m

The harness reports a table of pass/fail per scenario plus a
JSON dump under /tmp/turbo-mimicry/<scenario>.json. When invoked
with `--screenshot`, it also drives the SPA (Playwright) to
capture an overlay of both polylines on the Norwegian topo
basemap so the diagnostic is visual as well as numeric.

Usage:
  python3 tools/mimicry.py                # numbers-only
  python3 tools/mimicry.py --screenshot   # numbers + PNG
  python3 tools/mimicry.py --filter oslo  # subset by name
"""
from __future__ import annotations
import argparse
import json
import math
import os
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path

# --- config ------------------------------------------------------

HOST = os.environ.get("MIMICRY_HOST", "http://localhost:8090")
SPA_HOST = os.environ.get("MIMICRY_SPA", "http://localhost:5173")
OUT_DIR = Path("/tmp/turbo-mimicry")
OUT_DIR.mkdir(parents=True, exist_ok=True)

# --- TOML parsing (Python 3.11+) --------------------------------
try:
    import tomllib
except ImportError:
    import tomli as tomllib

# --- geometry helpers -------------------------------------------

def haversine_m(a: list[float], b: list[float]) -> float:
    """Great-circle distance between two [lon, lat] points in metres."""
    R = 6371000.0
    lon1, lat1 = math.radians(a[0]), math.radians(a[1])
    lon2, lat2 = math.radians(b[0]), math.radians(b[1])
    dlat = lat2 - lat1
    dlon = lon2 - lon1
    h = math.sin(dlat / 2) ** 2 + math.cos(lat1) * math.cos(lat2) * math.sin(dlon / 2) ** 2
    return 2 * R * math.asin(math.sqrt(h))

def polyline_length_m(pts: list[list[float]]) -> float:
    return sum(haversine_m(a, b) for a, b in zip(pts, pts[1:]))

def segment_lengths_m(pts: list[list[float]], densify_threshold_m: float = 5.0
                      ) -> list[float]:
    """Macro-segment lengths after coalescing densified intermediates.
    Returns the length of each *user-facing* segment — i.e., the
    distance between successive non-collinear waypoints. A path
    rendered as 250 vertices that's really 4 long LoS jumps shows
    up here as 4 lengths, not 250."""
    if len(pts) < 2:
        return []
    import math
    out = []
    anchor = pts[0]
    cum = 0.0
    for prev, cur in zip(pts, pts[1:]):
        cum += haversine_m(prev, cur)
        # Treat a vertex as a real waypoint when it deflects from the
        # straight line out of the anchor by more than the densify
        # threshold; otherwise it's just a polyline subdivision.
        # Approximate perpendicular distance using haversine.
        ax, ay = anchor; cx, cy = cur
        d_anchor = haversine_m(anchor, cur)
        if d_anchor == 0: continue
        # angle subtended
        d_seg = haversine_m(prev, cur)
        # Run a smoothing fold: if `cur` is roughly along anchor→prev
        # direction, keep extending; otherwise emit segment.
        bx, by = prev
        v1 = (bx-ax, by-ay)
        v2 = (cx-ax, cy-ay)
        n1 = math.hypot(*v1) or 1e-9
        n2 = math.hypot(*v2) or 1e-9
        dot = (v1[0]*v2[0] + v1[1]*v2[1]) / (n1*n2)
        # cos > 0.9995 ≈ within ~1.8° of straight
        if dot < 0.9995:
            out.append(cum - haversine_m(prev, cur))
            anchor = prev
            cum = haversine_m(prev, cur)
    out.append(cum)
    return [x for x in out if x > 1e-3]

def overshoot_metres(pts: list[list[float]]) -> float:
    """If the path passes its own destination and then comes back,
    return the max distance the path strays past the goal (measured
    radially from the last vertex). Zero when the path is monotonic
    in goal-distance through its final third."""
    if len(pts) < 5:
        return 0.0
    goal = pts[-1]
    # Distances from each vertex to the goal.
    dists = [haversine_m(p, goal) for p in pts]
    # Look only at the second half of the path — the overshoot
    # pattern is "approach, overshoot, return", and we want the gap
    # between the minimum reached before the end and the overshoot
    # peak that follows it.
    n = len(dists)
    half = n // 2
    tail = dists[half:]
    if len(tail) < 3:
        return 0.0
    # For each i in the tail, the overshoot is max(dists[i+1..end])
    # minus dists[i]. We want the largest such overshoot anywhere
    # in the tail. A monotonic-decreasing tail returns 0.
    worst = 0.0
    for i in range(len(tail) - 1):
        rest = max(tail[i+1:])
        worst = max(worst, rest - tail[i])
    return worst

def sharp_turns(pts: list[list[float]], threshold_deg: float = 75.0,
                min_segment_m: float = 30.0) -> tuple[int, float]:
    """Count sharp turning angles along a polyline and return the
    maximum turning angle (degrees). A 'sharp turn' is any inflection
    where the path deviates by more than `threshold_deg` from straight,
    measured between consecutive segments that are each at least
    `min_segment_m` long (so the densified rendering of a single
    LoS edge doesn't appear as a noisy series of micro-bends).
    Returns (n_sharp, max_turning_deg)."""
    import math
    def bearing(a: list[float], b: list[float]) -> float:
        lon1, lat1, lon2, lat2 = map(math.radians, [a[0], a[1], b[0], b[1]])
        dlon = lon2 - lon1
        y = math.sin(dlon) * math.cos(lat2)
        x = math.cos(lat1) * math.sin(lat2) - math.sin(lat1) * math.cos(lat2) * math.cos(dlon)
        return math.degrees(math.atan2(y, x))
    # Coalesce sub-min-segment vertices so we measure turns between
    # macro-segments, not the densified intermediates.
    coalesced: list[list[float]] = [pts[0]]
    for p in pts[1:]:
        if haversine_m(coalesced[-1], p) >= min_segment_m:
            coalesced.append(p)
    if coalesced[-1] is not pts[-1] and len(coalesced) > 1:
        coalesced.append(pts[-1])
    if len(coalesced) < 3:
        return 0, 0.0
    n_sharp = 0
    max_turn = 0.0
    for a, b, c in zip(coalesced, coalesced[1:], coalesced[2:]):
        b1 = bearing(a, b)
        b2 = bearing(b, c)
        diff = abs(b2 - b1)
        if diff > 180:
            diff = 360 - diff
        if diff > max_turn:
            max_turn = diff
        if diff > threshold_deg:
            n_sharp += 1
    return n_sharp, max_turn

def nearest_distance_to_polyline_m(p: list[float], line: list[list[float]]) -> float:
    """Brute-force nearest perpendicular distance from p to any segment
    of `line`. Quadratic in vertices but fine for our trail sizes."""
    best = math.inf
    for a, b in zip(line, line[1:]):
        # Project p onto segment ab in a small flat-earth tangent
        # plane. The scale factor at p.lat handles latitudinal
        # compression of longitude.
        ax, ay = a
        bx, by = b
        px, py = p
        scale = math.cos(math.radians(py))
        sxa = (ax - px) * scale
        sya = (ay - py)
        sxb = (bx - px) * scale
        syb = (by - py)
        dx = sxb - sxa
        dy = syb - sya
        len_sq = dx * dx + dy * dy
        if len_sq < 1e-12:
            d = math.hypot(sxa, sya)
        else:
            t = -(sxa * dx + sya * dy) / len_sq
            t = max(0.0, min(1.0, t))
            qx = sxa + t * dx
            qy = sya + t * dy
            d = math.hypot(qx, qy)
        # d is in approx degrees on a unit sphere; convert.
        d_m = d * 111_000.0
        if d_m < best:
            best = d_m
    return best if best != math.inf else 0.0

# --- HTTP -------------------------------------------------------

def post_pathfind(payload: dict) -> dict:
    req = urllib.request.Request(
        f"{HOST}/v1/pathfind",
        data=json.dumps(payload).encode(),
        headers={"content-type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=120) as resp:
        return json.load(resp)

def path_coords(p: dict) -> list[list[float]]:
    """Concatenate per-leg geometry, falling back to top-level
    geometry when present."""
    coords = []
    for leg in p.get("legs", []) or []:
        coords.extend(leg.get("geometry", []) or [])
    if not coords and p.get("geometry"):
        coords = p["geometry"]
    return coords

# --- one scenario ----------------------------------------------

def run_scenario(sc: dict, solver: str = "auto") -> dict:
    """Run baseline + offgrid for one scenario, compute metrics,
    write JSON, return a row for the summary table.

    `solver` selects the off-trail cost mode:
      - "auto" — server default (FastMarching from phase 8 onward).
      - "fmm"  — pin to `fast_marching` cost mode.
      - "theta"— pin to `walk_seconds` (legacy Theta\* path).
    """
    name = sc["name"]
    baseline_payload = {
        "from": sc["from"],
        "to":   sc["to"],
        "prefs": {"profile": sc.get("profile", "foot"), "snap_radius_m": 300},
    }
    offgrid_prefs = {
        "profile": sc.get("profile", "foot"),
        "snap_radius_m": 0,
        "bridge_radius_m": 0,
        "allow_off_trail": True,
        "force_off_trail": True,
        "max_off_trail_km": 20,
    }
    if solver == "fmm":
        offgrid_prefs["cost_mode"] = "fast_marching"
    elif solver == "theta":
        offgrid_prefs["cost_mode"] = "walk_seconds"
    # Optional per-request cost-config override for calibration
    # sweeps. The harness reads MIMICRY_OVERRIDE_JSON as a JSON
    # blob and threads it into `prefs.cost_config_override`. Lets
    # us A/B knob settings without restarting the tileserver.
    ovr_raw = os.environ.get("MIMICRY_OVERRIDE_JSON")
    if ovr_raw:
        try:
            offgrid_prefs["cost_config_override"] = json.loads(ovr_raw)
        except Exception:
            pass
    offgrid_payload = {
        "from": sc["from"],
        "to":   sc["to"],
        "prefs": offgrid_prefs,
    }
    t0 = time.time()
    try:
        baseline = post_pathfind(baseline_payload)
        offgrid  = post_pathfind(offgrid_payload)
    except Exception as e:
        return {"name": name, "error": str(e), "pass": False}
    dt = time.time() - t0

    bp = baseline.get("path", {})
    op = offgrid.get("path", {})
    bcoords = path_coords(bp)
    ocoords = path_coords(op)
    if not bcoords or not ocoords:
        return {"name": name, "error": "empty path",
                "baseline_pts": len(bcoords), "offgrid_pts": len(ocoords),
                "pass": False}

    # Distances every offgrid vertex to the baseline polyline.
    devs = [nearest_distance_to_polyline_m(p, bcoords) for p in ocoords]
    mean_dev = sum(devs) / len(devs)
    max_dev  = max(devs)
    b_len = polyline_length_m(bcoords)
    o_len = polyline_length_m(ocoords)
    ratio = o_len / b_len if b_len > 0 else math.nan

    # Shape metrics: count sharp turns and the maximum turning
    # angle. A 90° dogleg in the middle of an off-trail solve is a
    # smoking gun for "the algorithm picked two LoS jumps connected
    # at an arbitrary point" rather than a contour-following route.
    # Adding these makes the harness flag shape problems even when
    # the point-to-line distance is small.
    offgrid_sharp, offgrid_max_turn = sharp_turns(ocoords)
    baseline_sharp, baseline_max_turn = sharp_turns(bcoords)

    # Macro-segment lengths: the lengths between waypoints after
    # coalescing densified polyline intermediates. Surfaces "low-
    # poly" paths where the algorithm took 4 long LoS jumps. If
    # max_segment_m is hundreds of metres on a 3 km path through
    # uneven terrain, a human would never walk that.
    o_segs = segment_lengths_m(ocoords)
    avg_segment = (sum(o_segs) / len(o_segs)) if o_segs else 0.0
    max_segment = max(o_segs) if o_segs else 0.0
    # Overshoot: monotonic-violation on the final approach.
    overshoot = overshoot_metres(ocoords)

    row = {
        "name": name,
        "baseline_strategy": bp.get("strategy"),
        "offgrid_strategy":  op.get("strategy"),
        "baseline_length_m": round(b_len),
        "offgrid_length_m":  round(o_len),
        "length_ratio":      round(ratio, 3),
        "mean_dev_m":        round(mean_dev, 1),
        "max_dev_m":         round(max_dev, 1),
        "offgrid_sharp_turns": offgrid_sharp,
        "offgrid_max_turn_deg": round(offgrid_max_turn, 1),
        "baseline_sharp_turns": baseline_sharp,
        "baseline_max_turn_deg": round(baseline_max_turn, 1),
        "offgrid_segments": len(o_segs),
        "offgrid_avg_segment_m": round(avg_segment, 0),
        "offgrid_max_segment_m": round(max_segment, 0),
        "offgrid_overshoot_m": round(overshoot, 0),
        "wall_time_s":       round(dt, 2),
    }

    th_mean  = sc.get("mean_deviation_max_m", math.inf)
    th_max   = sc.get("max_deviation_max_m",  math.inf)
    th_ratio = sc.get("length_ratio_max",     math.inf)
    th_sharp = sc.get("sharp_turns_max",      math.inf)
    th_turn  = sc.get("max_turn_deg_max",     math.inf)
    th_segm  = sc.get("max_segment_m_max",    math.inf)
    th_over  = sc.get("overshoot_m_max",      math.inf)
    fails = []
    if mean_dev > th_mean:  fails.append(f"mean {mean_dev:.0f}>{th_mean:.0f}")
    if max_dev  > th_max:   fails.append(f"max  {max_dev:.0f}>{th_max:.0f}")
    if ratio    > th_ratio: fails.append(f"ratio {ratio:.2f}>{th_ratio:.2f}")
    if offgrid_sharp > th_sharp:
        fails.append(f"sharp {offgrid_sharp}>{th_sharp}")
    if offgrid_max_turn > th_turn:
        fails.append(f"turn {offgrid_max_turn:.0f}°>{th_turn:.0f}°")
    if max_segment > th_segm:
        fails.append(f"max_seg {max_segment:.0f}>{th_segm:.0f}")
    if overshoot > th_over:
        fails.append(f"overshoot {overshoot:.0f}>{th_over:.0f}")
    row["pass"] = not fails
    row["fail_reasons"] = fails

    # Dump everything (including polylines) for offline replay.
    dump = {
        "scenario": sc,
        "row": row,
        "baseline_geometry": bcoords,
        "offgrid_geometry":  ocoords,
    }
    (OUT_DIR / f"{name}.json").write_text(json.dumps(dump, indent=2))
    return row

# --- summary table ---------------------------------------------

def print_summary(rows: list[dict]) -> int:
    fail_count = sum(1 for r in rows if not r.get("pass", False))
    print()
    print(f"{'scenario':40s} {'strat':10s} {'len_m':>7s} {'rat':>4s} {'mean':>5s} {'max':>5s} "
          f"{'sharp':>5s} {'turn°':>5s} {'segs':>4s} {'max_s':>5s} {'over':>5s}  result")
    print("-" * 130)
    for r in rows:
        if "error" in r and not r.get("baseline_strategy"):
            print(f"{r['name']:40s} ERROR: {r['error']}")
            continue
        status = "OK" if r["pass"] else "FAIL"
        reasons = "  " + ", ".join(r.get("fail_reasons", []))
        print(
            f"{r['name']:40s} "
            f"{r.get('offgrid_strategy', '?'):10s} "
            f"{r.get('offgrid_length_m', 0):>7d} "
            f"{r.get('length_ratio', 0):>4.2f} "
            f"{r.get('mean_dev_m', 0):>5.0f} "
            f"{r.get('max_dev_m', 0):>5.0f} "
            f"{r.get('offgrid_sharp_turns', 0):>5d} "
            f"{r.get('offgrid_max_turn_deg', 0):>5.0f} "
            f"{r.get('offgrid_segments', 0):>4d} "
            f"{r.get('offgrid_max_segment_m', 0):>5.0f} "
            f"{r.get('offgrid_overshoot_m', 0):>5.0f}  "
            f"{status}{reasons if not r['pass'] else ''}"
        )
    print()
    print(f"summary: {len(rows)-fail_count}/{len(rows)} pass, {fail_count} fail")
    return fail_count

# --- screenshot driver -----------------------------------------

SCREENSHOT_JS = r"""
const { chromium } = require('/Users/sigmundsandring/StudioProjects/turbo/apps/admin/node_modules/playwright');
const fs = require('fs');

const cfg = JSON.parse(fs.readFileSync(process.argv[2], 'utf-8'));

(async () => {
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ viewport: { width: 1800, height: 1200 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await page.goto(`${cfg.spa}/admin/dev-login`, { waitUntil: 'domcontentloaded' });
  await page.goto(`${cfg.spa}/admin/app/plot`, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => window.__pf && window.__pf.map, { timeout: 20000 });

  // Compute centre + zoom from the union of both polylines.
  const all = [...cfg.baseline, ...cfg.offgrid];
  if (all.length === 0) { await browser.close(); return; }
  let minX = +Infinity, minY = +Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const [x, y] of all) {
    if (x < minX) minX = x; if (x > maxX) maxX = x;
    if (y < minY) minY = y; if (y > maxY) maxY = y;
  }
  await page.evaluate(([minX, minY, maxX, maxY]) => {
    window.__pf.map.fitBounds([[minX, minY], [maxX, maxY]], { padding: 60 });
  }, [minX, minY, maxX, maxY]);
  await page.waitForFunction(() => !window.__pf.map.isMoving() && window.__pf.map.isStyleLoaded(), { timeout: 15000 });
  await page.waitForTimeout(1500);

  // Inject both polylines as separate GeoJSON sources so the user
  // can compare baseline (green) vs offgrid (orange).
  await page.evaluate(({ baseline, offgrid }) => {
    const m = window.__pf.map;
    const addLine = (id, coords, color, width) => {
      if (m.getSource(id)) m.removeLayer(id);
      if (m.getSource(id)) m.removeSource(id);
      m.addSource(id, { type: 'geojson', data: { type: 'Feature', geometry: { type: 'LineString', coordinates: coords } } });
      m.addLayer({ id, type: 'line', source: id, paint: { 'line-color': color, 'line-width': width, 'line-opacity': 0.9 } });
    };
    addLine('mimicry-baseline', baseline, '#16a34a', 6);  // green = trail
    addLine('mimicry-offgrid',  offgrid,  '#f97316', 4);  // orange = mesh
  }, cfg);
  await page.waitForTimeout(1500);
  await page.screenshot({ path: cfg.out });
  await ctx.close();
  await browser.close();
})();
"""

def screenshot_scenario(name: str, baseline_geom: list, offgrid_geom: list) -> Path:
    cfg_path = OUT_DIR / f"{name}.shotcfg.json"
    out_path = OUT_DIR / f"{name}.png"
    cfg_path.write_text(json.dumps({
        "spa": SPA_HOST,
        "baseline": baseline_geom,
        "offgrid":  offgrid_geom,
        "out": str(out_path),
    }))
    script = OUT_DIR / "mimicry_screenshot.js"
    script.write_text(SCREENSHOT_JS)
    subprocess.run(["node", str(script), str(cfg_path)],
                   check=False, timeout=60,
                   cwd="/Users/sigmundsandring/StudioProjects/turbo/apps/admin")
    return out_path

# --- main ------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--filter", default=None, help="substring filter on scenario name")
    ap.add_argument("--screenshot", action="store_true", help="capture overlay PNGs via SPA")
    ap.add_argument("--corpus", default="tools/trail-mimicry.toml")
    ap.add_argument("--solver", choices=["auto", "fmm", "theta"], default="auto",
                    help="off-trail solver: auto=server default, fmm=fast_marching, theta=walk_seconds")
    args = ap.parse_args()

    corpus_path = Path(args.corpus)
    if not corpus_path.is_absolute():
        corpus_path = Path(__file__).parent.parent / args.corpus
    corpus = tomllib.loads(corpus_path.read_text())
    scenarios = corpus.get("scenario", [])
    if args.filter:
        scenarios = [s for s in scenarios if args.filter in s["name"]]
    if not scenarios:
        print(f"no scenarios match filter '{args.filter}'", file=sys.stderr)
        sys.exit(2)

    rows = []
    for sc in scenarios:
        print(f"  running {sc['name']} … ", end="", flush=True)
        row = run_scenario(sc, solver=args.solver)
        print("ok" if row.get("pass") else f"FAIL ({', '.join(row.get('fail_reasons', []) or [row.get('error', '?')])})")
        rows.append(row)

    fails = print_summary(rows)

    if args.screenshot:
        print("\ncapturing screenshots…")
        for sc in scenarios:
            name = sc["name"]
            d = OUT_DIR / f"{name}.json"
            if not d.exists():
                continue
            data = json.loads(d.read_text())
            out = screenshot_scenario(name, data["baseline_geometry"], data["offgrid_geometry"])
            print(f"  {name}: {out}")

    sys.exit(1 if fails else 0)

if __name__ == "__main__":
    main()
