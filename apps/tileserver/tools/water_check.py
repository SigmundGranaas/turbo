#!/usr/bin/env python3
"""Hard water-crossing gate for off-trail routes.

A route that crosses a refused cell (deep water / glacier / true cliff) is
an automatic FAIL, regardless of any composite score. For each route we
sample the polyline and ask the authoritative per-cell endpoint
(/v1/debug/pathfind/cell, which knows shoreline-passable vs deep-refused)
whether each point is refused.

Modes:
  --corpus                 run every terrain-corpus hike force-off-trail
  --route LON,LAT LON,LAT  check a single from/to pair
  --find-lake LON,LAT R_KM scan for a lake near a point, straddle it, check

All routes use force_off_trail so the grade-limited solver actually runs.
"""
import json, math, sys, urllib.request, tomllib

BASE = "http://localhost:8090"
STEP_M = 40.0  # route sampling interval for the crossing check


def post(path, body, timeout=60):
    r = urllib.request.Request(BASE + path, data=json.dumps(body).encode(),
                               headers={"content-type": "application/json"})
    return json.load(urllib.request.urlopen(r, timeout=timeout))


def hav(a, b):
    R = 6371000
    la1, lo1, la2, lo2 = map(math.radians, [a[1], a[0], b[1], b[0]])
    d1, d2 = la2 - la1, lo2 - lo1
    h = math.sin(d1 / 2) ** 2 + math.cos(la1) * math.cos(la2) * math.sin(d2 / 2) ** 2
    return 2 * R * math.asin(math.sqrt(h))


def route_off_trail(frm, to):
    pf = {"profile": "foot", "allow_off_trail": True, "max_off_trail_km": 50,
          "force_off_trail": True, "snap_radius_m": 0, "bridge_radius_m": 0}
    return post("/v1/pathfind", {"from": frm, "to": to, "prefs": pf})["path"]


def resample(coords, step_m):
    """Yield points spaced ~step_m along the polyline (incl. vertices)."""
    out = [coords[0]]
    acc = 0.0
    for i in range(len(coords) - 1):
        a, b = coords[i], coords[i + 1]
        seg = hav(a, b)
        if seg < 1e-6:
            continue
        d = step_m
        while d < seg:
            t = d / seg
            out.append([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
            d += step_m
        out.append(b)
    return out


# Hard refusals the route must NEVER cross (impassable). Slope is
# deliberately "steep-but-passable" in the 45–78° band to keep corridors
# connected, so it is tracked separately as soft, not a hard fail.
HARD = ("water", "ocean", "glacier", "mask", "lake", "sea")


def is_hard(rb: str) -> bool:
    return any(k in rb.lower() for k in HARD)


def crossings(coords):
    """Return (hard, soft) lists of (lon,lat,refused_by) sample points."""
    hard, soft = [], []
    for p in resample(coords, STEP_M):
        ci = post("/v1/debug/pathfind/cell", {"lon": p[0], "lat": p[1]})
        rb = ci.get("point", {}).get("refused_by")
        if rb:
            (hard if is_hard(rb) else soft).append((p[0], p[1], rb))
    return hard, soft


def check_route(name, frm, to):
    p = route_off_trail(frm, to)
    coords = p["geometry"]
    hard, soft = crossings(coords)
    L = sum(hav(coords[i], coords[i + 1]) for i in range(len(coords) - 1))
    status = "FAIL" if hard else "ok"

    def kinds(lst):
        k = {}
        for _, _, rb in lst:
            k[rb] = k.get(rb, 0) + 1
        return ", ".join(f"{a}×{b}" for a, b in k.items())

    extra = ""
    if hard:
        extra += "  HARD: " + kinds(hard)
    if soft:
        extra += f"  (soft slope×{len(soft)})"
    print(f"[{status}] {name:24s} strat={p['strategy']:9s} len={L:6.0f}m "
          f"pts={len(coords):5d}{extra}")
    return not hard


def main():
    args = sys.argv[1:]
    if args and args[0] == "--corpus":
        corpus = tomllib.loads(open("tools/terrain-corpus.toml").read())["hike"]
        ok = 0
        fails = []
        for h in corpus:
            name = f"{h['region']}-{h['id']}"
            try:
                if check_route(name, h["from"], h["to"]):
                    ok += 1
                else:
                    fails.append(name)
            except Exception as e:
                print(f"[ERR ] {name}: {e}")
                fails.append(name)
        print(f"\n=== {ok}/{len(corpus)} routes water-safe; "
              f"{len(fails)} crossed refused cells ===")
        if fails:
            print("FAILED:", ", ".join(fails))
            sys.exit(1)
    elif args and args[0] == "--route":
        frm = [float(x) for x in args[1].split(",")]
        to = [float(x) for x in args[2].split(",")]
        ok = check_route("route", frm, to)
        sys.exit(0 if ok else 1)
    else:
        print(__doc__)
        sys.exit(2)


if __name__ == "__main__":
    main()
