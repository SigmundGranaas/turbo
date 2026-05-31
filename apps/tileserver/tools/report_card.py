#!/usr/bin/env python3
"""Visual report cards for the terrain-routing eval (Phase A3).

Reads the per-hike JSON rows written by terrain_metrics.py and renders
a dependency-free SVG per hike plus an index.html. Each card overlays:

  - water raster cells (blue)  — from /v1/debug/data/water over the bbox
  - the ground-truth trail      (green)
  - the solver's route          (red)
  - start (○) and end (●) markers

and prints the metric battery. The point is to SEE whether a low score
is a legitimate obstacle detour (solver curves around water the trail
bridges) or a cost-field pathology (solver wanders / over-climbs on
open ground). We also probe /v1/mask/sample along the truth trail to
count how many ground-truth vertices sit on water — a trail that
crosses water explains an off-trail detour; one that doesn't means the
detour is the solver's own bad decision.

Usage:
  python3 tools/report_card.py --in /tmp/terrain-eval-fmm-offtrail \
      --out /tmp/terrain-cards --limit 20
"""
import argparse
import json
import math
import urllib.request
from pathlib import Path

HOST = "http://localhost:8090"


def get(path, params=None):
    url = f"{HOST}{path}"
    if params:
        from urllib.parse import urlencode
        url += "?" + urlencode(params)
    with urllib.request.urlopen(url, timeout=60) as r:
        return json.loads(r.read().decode())


def post(path, body):
    req = urllib.request.Request(
        f"{HOST}{path}", data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read().decode())


def metres(a, b):
    return math.hypot((b[0] - a[0]) * 55000.0, (b[1] - a[1]) * 111000.0)


def water_cells(bbox):
    try:
        r = get("/v1/debug/data/water", {
            "west": bbox[0], "south": bbox[1], "east": bbox[2], "north": bbox[3]})
        return r.get("cells", [])
    except Exception:
        return []


def truth_water_crossings(poly, step=8):
    """Count truth vertices that sample as water. Subsample for speed."""
    n = 0
    tot = 0
    for i in range(0, len(poly), step):
        lon, lat = poly[i]
        try:
            r = post("/v1/mask/sample", {"lon": lon, "lat": lat})
            tot += 1
            if r.get("kind") == "water" or r.get("refused"):
                n += 1
        except Exception:
            pass
    return n, tot


def svg_card(row, water, out_path):
    truth = row["truth_poly"]
    solver = row["solver_poly"]
    allpts = truth + solver + [[c[0], c[1]] for c in water]
    xs = [p[0] for p in allpts]
    ys = [p[1] for p in allpts]
    minx, maxx = min(xs), max(xs)
    miny, maxy = min(ys), max(ys)
    # pad
    padx = (maxx - minx) * 0.05 or 0.001
    pady = (maxy - miny) * 0.05 or 0.001
    minx -= padx; maxx += padx; miny -= pady; maxy += pady
    W, H = 700, 700
    # latitude aspect correction: 1 deg lon ~ cos(lat) of 1 deg lat
    lat0 = (miny + maxy) / 2
    aspect = math.cos(math.radians(lat0))
    spanx = (maxx - minx) * aspect
    spany = (maxy - miny)
    span = max(spanx, spany)

    def X(lon):
        return (lon - minx) * aspect / span * W
    def Y(lat):
        return H - (lat - miny) / span * H

    def path_d(poly):
        return "M " + " L ".join(f"{X(p[0]):.1f},{Y(p[1]):.1f}" for p in poly)

    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
             f'style="background:#f7f7f4">']
    # water cells as small squares
    for c in water:
        cx, cy = X(c[0]), Y(c[1])
        parts.append(f'<rect x="{cx-1.2:.1f}" y="{cy-1.2:.1f}" width="2.4" height="2.4" '
                     f'fill="#7fb4e6" opacity="0.6"/>')
    # truth + solver
    parts.append(f'<path d="{path_d(truth)}" fill="none" stroke="#1a8f3c" '
                 f'stroke-width="3" opacity="0.9"/>')
    parts.append(f'<path d="{path_d(solver)}" fill="none" stroke="#d62728" '
                 f'stroke-width="2" opacity="0.85"/>')
    # markers
    s = truth[0]; e = truth[-1]
    parts.append(f'<circle cx="{X(s[0]):.1f}" cy="{Y(s[1]):.1f}" r="6" '
                 f'fill="none" stroke="#000" stroke-width="2"/>')
    parts.append(f'<circle cx="{X(e[0]):.1f}" cy="{Y(e[1]):.1f}" r="6" fill="#000"/>')
    parts.append('</svg>')
    out_path.write_text("\n".join(parts))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="indir", default="/tmp/terrain-eval-fmm-offtrail")
    ap.add_argument("--out", default="/tmp/terrain-cards")
    ap.add_argument("--limit", type=int, default=20, help="worst-N by score")
    args = ap.parse_args()

    indir = Path(args.indir)
    outdir = Path(args.out)
    outdir.mkdir(parents=True, exist_ok=True)
    rows = []
    for f in indir.glob("*.json"):
        if f.name == "_summary.json":
            continue
        rows.append(json.loads(f.read_text()))
    rows.sort(key=lambda r: r["score"])
    rows = rows[:args.limit]

    cards = []
    for r in rows:
        name = r["name"]
        truth = r["truth_poly"]; solver = r["solver_poly"]
        xs = [p[0] for p in truth + solver]
        ys = [p[1] for p in truth + solver]
        bbox = (min(xs), min(ys), max(xs), max(ys))
        water = water_cells(bbox)
        wn, wtot = truth_water_crossings(truth)
        svg_card(r, water, outdir / f"{name}.svg")
        tl = sum(metres(truth[i], truth[i+1]) for i in range(len(truth)-1))
        sl = sum(metres(solver[i], solver[i+1]) for i in range(len(solver)-1))
        t = r["truth"]; s = r["solver"]
        verdict = ("water-detour?" if wn > 0 else "open-ground")
        cards.append({
            "name": name, "score": r["score"],
            "len_ratio": round(sl / tl, 2) if tl else 0,
            "truth_gain": t["elev_gain_m"], "solver_gain": s["elev_gain_m"],
            "truth_maxslope": t["max_slope_deg"], "solver_maxslope": s["max_slope_deg"],
            "npts": len(solver), "water_cells": len(water),
            "truth_water_verts": f"{wn}/{wtot}", "verdict": verdict,
        })
        print(f"{name:<22} score={r['score']:>5}  lenx{cards[-1]['len_ratio']:<5} "
              f"gain {t['elev_gain_m']:>5}->{s['elev_gain_m']:<6} "
              f"twater={wn}/{wtot:<3} {verdict}")

    # index.html
    html = ["<html><head><meta charset='utf-8'><style>",
            "body{font-family:system-ui;margin:20px;background:#fff}",
            ".card{display:inline-block;vertical-align:top;margin:10px;"
            "border:1px solid #ccc;border-radius:6px;padding:8px;width:360px}",
            ".card img{width:340px;height:340px;border:1px solid #eee}",
            "table{font-size:12px;border-collapse:collapse}td{padding:1px 6px}",
            ".legend{margin:6px 0;font-size:13px}",
            "</style></head><body>",
            "<h2>Off-trail FMM report cards (worst by score)</h2>",
            "<div class='legend'>"
            "<span style='color:#1a8f3c'>━ ground-truth trail</span> &nbsp; "
            "<span style='color:#d62728'>━ solver route</span> &nbsp; "
            "<span style='color:#7fb4e6'>■ water</span> &nbsp; ○ start ● end</div>"]
    for c in cards:
        html.append("<div class='card'>")
        html.append(f"<b>{c['name']}</b> — score {c['score']}<br>")
        html.append(f"<img src='{c['name']}.svg'>")
        html.append("<table>"
                     f"<tr><td>len ratio</td><td>{c['len_ratio']}×</td></tr>"
                     f"<tr><td>gain (truth→solver)</td><td>{c['truth_gain']}→{c['solver_gain']} m</td></tr>"
                     f"<tr><td>max slope</td><td>{c['truth_maxslope']}°→{c['solver_maxslope']}°</td></tr>"
                     f"<tr><td>solver npts</td><td>{c['npts']}</td></tr>"
                     f"<tr><td>truth verts on water</td><td>{c['truth_water_verts']}</td></tr>"
                     f"<tr><td>verdict</td><td><b>{c['verdict']}</b></td></tr>"
                     "</table></div>")
    html.append("</body></html>")
    (outdir / "index.html").write_text("\n".join(html))
    print(f"\nWrote {len(cards)} cards + index.html to {outdir}")


if __name__ == "__main__":
    main()
