#!/usr/bin/env python3
"""
DEM-coverage diagnostic. Probes the running tileserver's DEM at a
regular grid over the country's UTM33N extent and reports per-tile
nodata fraction, both as a textual summary and an SVG heatmap.

Purpose: when an off-trail solve goes straight through what should
be impassable terrain, the usual cause is the DEM having no data
at that point — every slope/Naismith/contour contributor silently
returns 0 contribution for nodata samples. This tool maps where
those holes are so they can be filled by reingesting from
Kartverket's DTM10/DTM50 sources.

Usage:
  python3 tools/dem_gaps.py             # summary only
  python3 tools/dem_gaps.py --svg out.svg
  python3 tools/dem_gaps.py --bbox lonW,latS,lonE,latN
"""
from __future__ import annotations
import argparse
import json
import sys
import urllib.request

HOST = "http://localhost:8090"

# Default probe bbox: continental Norway in WGS84 lon/lat. Extends
# from Lindesnes (58°N, southern tip) to North Cape (71°N).
DEFAULT_BBOX = (4.0, 58.0, 31.0, 71.0)

# Probe resolution. 30×30 = 900 points; each costs one HTTP roundtrip
# (~5 ms over loopback). At ~5 s total for a Norway-wide scan that's
# a reasonable interactive turnaround.
GRID_W = 30
GRID_H = 30


def sample_one(lon: float, lat: float) -> float | None:
    """Return elevation at (lon, lat), or None if DEM has no data."""
    req = urllib.request.Request(
        f"{HOST}/v1/elev/sample",
        data=json.dumps({"lon": lon, "lat": lat}).encode(),
        headers={"content-type": "application/json"},
    )
    try:
        return json.load(urllib.request.urlopen(req, timeout=5)).get("elev_m")
    except Exception:
        return None

def sample_mask(lon: float, lat: float) -> str:
    """Return mask classification at (lon, lat). Used to distinguish
    "ocean — no DEM is correct" from "land — DEM gap is a real problem"."""
    req = urllib.request.Request(
        f"{HOST}/v1/mask/sample",
        data=json.dumps({"lon": lon, "lat": lat}).encode(),
        headers={"content-type": "application/json"},
    )
    try:
        return json.load(urllib.request.urlopen(req, timeout=5)).get("kind", "unknown")
    except Exception:
        return "unknown"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bbox", default=None,
                    help="W,S,E,N in WGS84 lon/lat (overrides default Norway bbox)")
    ap.add_argument("--svg", default=None, help="write a heat-map SVG to this path")
    ap.add_argument("--grid", default=None,
                    help="probe grid as WxH (default 30x30)")
    args = ap.parse_args()

    if args.bbox:
        w, s, e, n = [float(x) for x in args.bbox.split(",")]
    else:
        w, s, e, n = DEFAULT_BBOX
    gw, gh = GRID_W, GRID_H
    if args.grid:
        gw, gh = [int(x) for x in args.grid.split("x")]

    print(f"probing DEM over WGS84 bbox W={w} S={s} E={e} N={n} "
          f"at {gw}×{gh} = {gw*gh} samples …", file=sys.stderr)

    # Grid scan
    grid: list[list[bool]] = [[False] * gw for _ in range(gh)]
    present = missing = 0
    for r in range(gh):
        for c in range(gw):
            lon = w + (e - w) * (c + 0.5) / gw
            lat = s + (n - s) * (gh - r - 0.5) / gh  # invert so row 0 = top
            elev = sample_one(lon, lat)
            grid[r][c] = elev is not None
            if grid[r][c]:
                present += 1
            else:
                missing += 1
        # Progress dot per row
        print(".", end="", flush=True, file=sys.stderr)
    print(file=sys.stderr)

    pct = 100.0 * present / (present + missing) if (present + missing) else 0.0
    print(f"\nDEM coverage (rectangular probe — includes ocean + other countries):")
    print(f"  {present}/{present + missing} samples present ({pct:.1f}%); {missing} missing")
    print(f"  bbox: W={w} S={s} E={e} N={n}")
    print(f"  grid: {gw}×{gh}")
    print(f"  ⚠  Norway occupies ~1/3 of this rectangle. The remaining 2/3 is")
    print(f"     ocean / Sweden / Finland / Russia — DEM is correctly absent there.")

    # Second probe: 50 named anchors (real Norwegian places). This is
    # the coverage that actually matters — if an anchor is missing DEM,
    # it's a genuine gap in the source data, not a probe-geometry quirk.
    print(f"\nDEM coverage on known Norwegian land points (named anchors):")
    try:
        anchors_req = urllib.request.Request(
            f"{HOST}/v1/debug/anchors/sample?limit=100",
            method="GET",
        )
        anchors = json.load(urllib.request.urlopen(anchors_req, timeout=10))
    except Exception:
        anchors = None
    if not anchors:
        # Fallback: hardcoded list spanning the country if the
        # anchors-sample endpoint isn't available.
        anchors = {"anchors": [
            {"name":"Lindesnes",          "lon": 7.050, "lat": 57.983},
            {"name":"Stavanger",          "lon": 5.730, "lat": 58.970},
            {"name":"Bergen",             "lon": 5.330, "lat": 60.390},
            {"name":"Oslo",               "lon":10.750, "lat": 59.910},
            {"name":"Trysil",             "lon":12.270, "lat": 61.310},
            {"name":"Spiterstulen",       "lon": 8.395, "lat": 61.624},
            {"name":"Trondheim",          "lon":10.395, "lat": 63.430},
            {"name":"Bodo",               "lon":14.365, "lat": 67.270},
            {"name":"Tromso",             "lon":18.919, "lat": 69.683},
            {"name":"Alta",               "lon":23.300, "lat": 69.970},
            {"name":"Karasjok",           "lon":25.500, "lat": 69.470},
            {"name":"Kirkenes",           "lon":29.733, "lat": 69.726},
            {"name":"Nordkapp",           "lon":25.783, "lat": 71.170},
        ]}
    land_total = 0
    land_present = 0
    land_missing: list[tuple[str, float, float]] = []
    for a in anchors.get("anchors", []):
        lon = a.get("lon") if "lon" in a else None
        lat = a.get("lat") if "lat" in a else None
        if lon is None or lat is None:
            continue
        # Skip water-feature anchors (lakes, fjords) — DEM under water
        # is irrelevant to whether routing has terrain awareness on land.
        if a.get("kind") in ("waterfeature",):
            continue
        e = sample_one(lon, lat)
        land_total += 1
        if e is not None:
            land_present += 1
        else:
            land_missing.append((a.get("name", "?"), lon, lat))
    if land_total:
        lpct = 100.0 * land_present / land_total
        print(f"  {land_present}/{land_total} anchors covered ({lpct:.1f}%)")
        if land_missing:
            print(f"  REAL GAPS at {len(land_missing)} land anchors:")
            for name, lon, lat in land_missing[:15]:
                print(f"    {name}  @ ({lon:.4f}, {lat:.4f})")

    # Banded summary so you can see which latitude bands are dead.
    print("\nlatitude-band coverage (rows 0 = north):")
    for r in range(gh):
        row_present = sum(1 for v in grid[r] if v)
        bar = "█" * row_present + "·" * (gw - row_present)
        lat_top = n - (n - s) * r / gh
        lat_bot = n - (n - s) * (r + 1) / gh
        print(f"  lat {lat_top:5.2f}° → {lat_bot:5.2f}°  |{bar}|  {row_present}/{gw}")

    if args.svg:
        cell_px = 14
        svg = []
        svg.append(f'<svg xmlns="http://www.w3.org/2000/svg" '
                   f'width="{gw*cell_px + 250}" height="{gh*cell_px + 100}" '
                   f'viewBox="0 0 {gw*cell_px + 250} {gh*cell_px + 100}">')
        svg.append('<style>.label{font:11px monospace}.title{font:14px sans-serif;font-weight:bold}</style>')
        svg.append(f'<text class="title" x="10" y="22">Norway DEM coverage  '
                   f'— {present}/{present+missing} ({pct:.1f}%) present</text>')
        for r in range(gh):
            lat = n - (n - s) * (r + 0.5) / gh
            for c in range(gw):
                x = c * cell_px + 50
                y = r * cell_px + 40
                color = "#34d399" if grid[r][c] else "#dc2626"
                svg.append(f'<rect x="{x}" y="{y}" width="{cell_px-1}" '
                           f'height="{cell_px-1}" fill="{color}"/>')
            svg.append(f'<text class="label" x="{gw*cell_px + 60}" '
                       f'y="{r*cell_px + 50}">lat {lat:5.2f}°</text>')
        svg.append('<text class="label" x="10" y="' + str(gh*cell_px + 60) +
                   '">green = DEM present · red = DEM nodata</text>')
        svg.append('</svg>')
        with open(args.svg, "w") as f:
            f.write("\n".join(svg))
        print(f"\nheat-map written to {args.svg}")


if __name__ == "__main__":
    main()
