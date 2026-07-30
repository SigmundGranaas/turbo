#!/usr/bin/env python3
"""Compare two eval-terrain runs on routing QUALITY, not just hashes.

    python3 tools/calibration_diff.py <baseline_out_dir> <variant_out_dir>

The routing gate answers "did anything move?" — the right question for a
refactor, and a deliberately blunt one. Phase E changes move geometry ON
PURPOSE, so the gate can only say "yes, as expected". This answers the
question that actually decides whether to keep such a change: did the
routes get *better*?

Metrics, per hike, against the walked ground truth:

  dev_m       Mean distance from each solver vertex to the truth
              polyline. The headline number: how far the solver's idea
              of the route is from where people actually walk.
  length_m    Route length. A solver that shortcuts through a lake
              scores well on deviation and badly here.
  gain_m      Positive elevation gain. Hikers minimise climb; a change
              that adds gain is usually making worse decisions even if
              deviation is flat.

Reported as before → after, plus the per-hike distribution — because an
average hides the shape. A knob that rescues 7 bad routes while
degrading 40 good ones nets positive and is still the wrong change, and
only the win/loss split shows it.
"""

import json
import math
import pathlib
import sys


# The eval harness emits both polylines in WGS84 degrees, so distances
# have to be metricated before they mean anything. A local equirectangular
# projection is ample here: the corpus spans under a degree of latitude,
# and we are comparing two routes over the same ground, not surveying.
LAT0 = 67.0
M_PER_DEG_LAT = 111_132.0
M_PER_DEG_LON = 111_320.0 * math.cos(math.radians(LAT0))


def to_m(pt):
    return (pt[0] * M_PER_DEG_LON, pt[1] * M_PER_DEG_LAT)


def seg_dist(p, a, b):
    """Distance from point p to segment ab, planar metres."""
    (px, py), (ax, ay), (bx, by) = p, a, b
    dx, dy = bx - ax, by - ay
    den = dx * dx + dy * dy
    if den < 1e-12:
        return math.hypot(px - ax, py - ay)
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / den))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def mean_deviation(solver, truth):
    """Mean distance from solver vertices to the truth polyline.

    Vertex-to-polyline rather than vertex-to-vertex: the two are sampled
    differently, so pairing them by index would measure the sampling and
    not the route.
    """
    if len(solver) < 1 or len(truth) < 2:
        return None
    solver = [to_m(p) for p in solver]
    truth = [to_m(p) for p in truth]
    total = 0.0
    for p in solver:
        total += min(seg_dist(p, truth[i], truth[i + 1]) for i in range(len(truth) - 1))
    return total / len(solver)


def gain(elevs):
    g = 0.0
    prev = None
    for e in elevs:
        if e is None:
            continue
        if prev is not None and e > prev:
            g += e - prev
        prev = e
    return g


def load(d):
    out = {}
    for f in pathlib.Path(d).glob("*.json"):
        if f.name == "_summary.json":
            continue
        r = json.loads(f.read_text())
        if not r.get("ok"):
            out[r["id"]] = None
            continue
        s, t = r["solver"], r["truth"]
        out[r["id"]] = {
            "dev_m": mean_deviation(s["polyline"], t["polyline"]),
            "length_m": s["length_m"],
            "gain_m": gain(s["elev_m"]),
            "truth_length_m": t["length_m"],
        }
    return out


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    base, var = load(sys.argv[1]), load(sys.argv[2])

    common = [k for k in base if k in var and base[k] and var[k]]
    only_base = [k for k in base if base[k] and not var.get(k)]
    only_var = [k for k in var if var[k] and not base.get(k)]

    if only_base:
        print(f"REGRESSION: {len(only_base)} hikes solved before and fail now: {only_base[:8]}")
    if only_var:
        print(f"newly solved: {len(only_var)} hikes: {only_var[:8]}")
    if not common:
        print("no hikes solved in both runs")
        return 1

    print(f"\n{len(common)} hikes solved in both\n")
    print(f"{'metric':<12} {'before':>10} {'after':>10} {'delta':>10} {'better':>8} {'worse':>7}")
    for key, lower_is_better in (("dev_m", True), ("length_m", True), ("gain_m", True)):
        b = sum(base[k][key] for k in common) / len(common)
        v = sum(var[k][key] for k in common) / len(common)
        better = sum(1 for k in common if (var[k][key] < base[k][key]) == lower_is_better
                     and abs(var[k][key] - base[k][key]) > 1e-6)
        worse = sum(1 for k in common if (var[k][key] > base[k][key]) == lower_is_better
                    and abs(var[k][key] - base[k][key]) > 1e-6)
        print(f"{key:<12} {b:>10.1f} {v:>10.1f} {v - b:>+10.1f} {better:>8} {worse:>7}")

    moved = sorted(common, key=lambda k: var[k]["dev_m"] - base[k]["dev_m"])
    print("\nmost improved (deviation):")
    for k in moved[:5]:
        print(f"  {k:>7}  {base[k]['dev_m']:>7.1f} -> {var[k]['dev_m']:>7.1f} m")
    print("most degraded (deviation):")
    for k in moved[-5:][::-1]:
        print(f"  {k:>7}  {base[k]['dev_m']:>7.1f} -> {var[k]['dev_m']:>7.1f} m")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
