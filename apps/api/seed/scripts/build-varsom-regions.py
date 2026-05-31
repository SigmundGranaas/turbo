#!/usr/bin/env python3
"""Build seed/varsom-regions.geojson from NVE's Snøskredvarsel API.

The Snøskredvarsel API exposes Norway's 24 avalanche-warning regions
plus 22 inland flood / landslide regions. We only want the avalanche
ones (TypeName=A) — the polygons are the same set the BackcountrySki
orchestrator looks up via ST_Contains.

Run:
    python3 build-varsom-regions.py [OUT_PATH]

Default OUT_PATH is ../varsom-regions.geojson relative to this script,
which matches the location the seeder config in appsettings expects.
The script is idempotent — re-run whenever the upstream region
taxonomy changes (very rare; last meaningful redraw was 2013).
"""
from __future__ import annotations

import json
import os
import sys
import urllib.request
from pathlib import Path

API_URL = "https://api01.nve.no/hydrology/forecast/avalanche/v6.3.0/api/Region"


def parse_ring(s: str) -> list[list[float]]:
    """Parse a polygon ring from the Varsom API's "lat lng, lat lng, ..." string format.

    The API returns each polygon ring as a single space- and comma-separated
    string of lat/lng pairs. GeoJSON expects [lng, lat] in each coordinate
    tuple and a closed ring; we handle both.
    """
    coords: list[list[float]] = []
    for pt in s.split(","):
        parts = pt.strip().split()
        if len(parts) != 2:
            continue
        lat = float(parts[0])
        lng = float(parts[1])
        coords.append([lng, lat])
    if coords and coords[0] != coords[-1]:
        coords.append(coords[0])
    return coords


def main(argv: list[str]) -> int:
    out_path = (
        Path(argv[1])
        if len(argv) > 1
        else Path(__file__).resolve().parent.parent / "varsom-regions.geojson"
    )

    print(f"Fetching {API_URL} ...", file=sys.stderr)
    with urllib.request.urlopen(API_URL, timeout=30) as resp:
        raw = json.load(resp)

    features = []
    for region in raw:
        if region.get("TypeName") != "A":
            continue
        rings = [parse_ring(p) for p in region.get("Polygon", []) if p]
        rings = [r for r in rings if len(r) >= 4]
        if not rings:
            continue
        if len(rings) == 1:
            geom = {"type": "Polygon", "coordinates": [rings[0]]}
        else:
            geom = {"type": "MultiPolygon", "coordinates": [[r] for r in rings]}
        features.append({
            "type": "Feature",
            "properties": {
                "OmradeId": region["Id"],
                "OmradeNavn": region["Name"],
                "TypeName": region["TypeName"],
            },
            "geometry": geom,
        })

    fc = {"type": "FeatureCollection", "features": features}
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(fc, f, ensure_ascii=False)
    print(f"Wrote {len(features)} regions to {out_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
