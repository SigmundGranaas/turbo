#!/usr/bin/env python3
"""Render a route over a slope-shaded DEM so the steep terrain it avoids
is visible. For each hike: sample an elevation grid over the bbox (one
/v1/elev/profile call per latitude row), compute slope in degrees, paint
a green→red slope heatmap, and overlay the solver route (force-off-trail)
plus the ground-truth trail.

Usage: python3 tools/slope_render.py <id-or-name> [<id> ...]
       python3 tools/slope_render.py --all   (the review set)
"""
import json, math, sys, urllib.request, tomllib
import numpy as np
from PIL import Image, ImageDraw, ImageFont

BASE = "http://localhost:8090"
GRID = 90          # grid resolution (cells per side)
SCALE = 9          # px per cell in output
PAD = 0.35         # bbox padding as fraction of endpoint span


def post(path, body):
    # TURBO_DEV_AUTH=1 on the dev server bypasses auth — no token needed.
    r = urllib.request.Request(BASE + path, data=json.dumps(body).encode(),
                               headers={"content-type": "application/json"})
    return json.load(urllib.request.urlopen(r, timeout=60))


def route(frm, to, force=True):
    pf = {"profile": "foot", "allow_off_trail": True, "max_off_trail_km": 50}
    if force:
        pf.update(force_off_trail=True, snap_radius_m=0, bridge_radius_m=0)
    return post("/v1/pathfind", {"from": frm, "to": to, "prefs": pf})["path"]


def elev_grid(lon0, lon1, lat0, lat1, n):
    """Return (n,n) elevation array, row 0 = north (lat1), nodata=nan."""
    rows = []
    lats = np.linspace(lat1, lat0, n)        # north -> south
    for la in lats:
        j = post("/v1/elev/profile",
                 {"line": [[lon0, la], [lon1, la]], "samples": n})
        rows.append([np.nan if z is None else z for z in j["elev_m"]])
    return np.array(rows, float)


def slope_deg(elev, lat_mid, lon0, lon1, lat0, lat1, n):
    dlat_m = abs(lat1 - lat0) * 111320.0 / (n - 1)
    dlon_m = abs(lon1 - lon0) * 111320.0 * math.cos(math.radians(lat_mid)) / (n - 1)
    # fill nodata with the global mean for the gradient pass, then re-mask
    # the gaps so they paint as gray (only affects gradient near gaps).
    filled = elev.copy()
    mask = np.isnan(filled)
    if mask.any():
        filled[mask] = np.nanmean(filled)
    gy, gx = np.gradient(filled, dlat_m, dlon_m)
    s = np.degrees(np.arctan(np.hypot(gx, gy)))
    s[mask] = np.nan
    return s


def slope_color(s):
    """Green (flat) -> yellow (~20) -> orange (~30) -> red (>40). nan=gray."""
    h, w = s.shape
    img = np.zeros((h, w, 3), np.uint8)
    stops = [(0, (60, 140, 60)), (15, (170, 200, 70)), (22, (235, 215, 80)),
             (27, (240, 160, 50)), (35, (220, 90, 40)), (50, (150, 25, 25))]
    for i in range(h):
        for jx in range(w):
            v = s[i, jx]
            if math.isnan(v):
                img[i, jx] = (90, 90, 100)
                continue
            for k in range(len(stops) - 1):
                a, ca = stops[k]; b, cb = stops[k + 1]
                if v <= b or k == len(stops) - 2:
                    t = max(0, min(1, (v - a) / (b - a))) if b > a else 0
                    img[i, jx] = tuple(int(ca[m] + (cb[m] - ca[m]) * t) for m in range(3))
                    break
    return img


def to_px(lon, lat, lon0, lon1, lat0, lat1, W, H):
    x = (lon - lon0) / (lon1 - lon0) * W
    y = (lat1 - lat) / (lat1 - lat0) * H
    return (x, y)


def render(name, frm, to, gt):
    span_lon = abs(to[0] - frm[0]); span_lat = abs(to[1] - frm[1])
    cx = (frm[0] + to[0]) / 2; cy = (frm[1] + to[1]) / 2
    half_lon = max(span_lon, span_lat / math.cos(math.radians(cy))) / 2 * (1 + 2 * PAD) + 0.0015
    half_lat = half_lon * math.cos(math.radians(cy))
    lon0, lon1 = cx - half_lon, cx + half_lon
    lat0, lat1 = cy - half_lat, cy + half_lat

    elev = elev_grid(lon0, lon1, lat0, lat1, GRID)
    s = slope_deg(elev, cy, lon0, lon1, lat0, lat1, GRID)
    rgb = slope_color(s)
    W = H = GRID * SCALE
    img = Image.fromarray(rgb).resize((W, H), Image.NEAREST)
    dr = ImageDraw.Draw(img)

    pth = route(frm, to)
    geo = pth["geometry"]; strat = pth["strategy"]

    def draw_line(pts, color, width, casing=None):
        px = [to_px(p[0], p[1], lon0, lon1, lat0, lat1, W, H) for p in pts]
        if casing:
            dr.line(px, fill=casing, width=width + 4, joint="curve")
        dr.line(px, fill=color, width=width, joint="curve")

    if gt:
        draw_line(gt, (20, 20, 20), 3, casing=(255, 255, 255))   # ground truth: black/white
    draw_line(geo, (0, 120, 255), 4, casing=(255, 255, 255))     # solver: blue

    def dot(p, col):
        x, y = to_px(p[0], p[1], lon0, lon1, lat0, lat1, W, H)
        dr.ellipse([x - 7, y - 7, x + 7, y + 7], fill=col, outline=(255, 255, 255), width=2)
    dot(frm, (40, 200, 40)); dot(to, (220, 40, 40))

    # legend
    dr.rectangle([6, 6, 360, 70], fill=(255, 255, 255))
    dr.text((12, 10), f"{name}  [{strat}]", fill=(0, 0, 0))
    dr.text((12, 26), "slope: green<15  yellow~22  orange~27  red>40deg  gray=nodata", fill=(0, 0, 0))
    dr.text((12, 42), "BLUE=solver route   BLACK/WHITE=real trail   green=start red=end", fill=(0, 0, 0))
    out = f"/tmp/slope-{name}.png"
    img.save(out)
    # quick stats
    finite = s[~np.isnan(s)]
    print(f"{name}: {strat} grid_slope max={np.nanmax(s):.0f} p90={np.percentile(finite,90):.0f} nodata={np.isnan(s).mean()*100:.0f}% -> {out}")


REVIEW = {
    "heggmotinden": ([14.848644, 67.366542], [14.880441, 67.376157], None),
}


def main():
    corpus = {str(h["id"]): h for h in tomllib.loads(open("tools/terrain-corpus.toml").read())["hike"]}
    args = sys.argv[1:]
    if args == ["--all"]:
        ids = ["3304880", "1948615", "3375156", "4220456"]
        targets = [(f"{corpus[i]['region']}-{i}", corpus[i]["from"], corpus[i]["to"], corpus[i]["polyline"]) for i in ids]
        targets.insert(0, ("heggmotinden", *REVIEW["heggmotinden"]))
    else:
        targets = []
        for a in args:
            if a in corpus:
                h = corpus[a]; targets.append((f"{h['region']}-{a}", h["from"], h["to"], h["polyline"]))
            elif a in REVIEW:
                targets.append((a, *REVIEW[a]))
    for name, frm, to, gt in targets:
        render(name, frm, to, gt)


if __name__ == "__main__":
    main()
