#!/usr/bin/env python3
"""Sample real hiker-walked trail polylines from paths.edge as the
ground truth for the terrain-routing evaluation harness.

Each output row is one "hike" — a single sti edge from the database
that is long enough to be a meaningful routing problem (1-10 km).
The edge's polyline IS the ground truth: it's what a real human
walked, encoded in the FKB-Veg `sti` ingest.

Sampling strategy:
  - Length 1-10 km (filters out tiny stubs and absurdly long single
    edges that are likely ingest artefacts)
  - Geographic diversity: sample within latitude bands so we cover
    Norway from south (Oslo region) to north (Lofoten/Finnmark)
  - Terrain diversity: bucket by mean slope (flat / rolling / steep)
    and ensure each bucket has representation

Output: tools/terrain-corpus.toml with [[hike]] entries containing
from_lon, from_lat, to_lon, to_lat, length_m, elevation_gain_m,
mean_slope_deg, region_label, ground_truth_polyline (list of
[lon, lat] pairs).
"""
import json
import subprocess
import sys
from pathlib import Path
import tomllib

PSQL = "/opt/homebrew/opt/libpq/bin/psql"
DB = "postgres://postgres:yourpassword@localhost:5446/tiles"
OUT = Path(__file__).parent / "terrain-corpus.toml"

# 50 hikes total: 5 latitude bands × 3 slope buckets × ~3 each + filler.
LAT_BANDS = [
    ("south",     58.5, 60.5),   # Oslo, Bergen
    ("midwest",   60.5, 62.5),   # Jotunheimen, Sognefjord
    ("trondelag", 62.5, 64.5),   # Trondheim
    ("nordland",  65.5, 68.5),   # Bodø, Lofoten, Narvik
    ("finnmark",  68.5, 71.0),   # Tromsø, Alta
]
# Slope + elevation_gain aren't baked in this DB build. Sample by
# region only; terrain bucket gets enriched downstream by querying
# the DEM via /v1/elev along each polyline.
N_PER_REGION = 12  # 5 × 12 = 60 candidates total

QUERY = """
WITH cands AS (
  SELECT
    id,
    length_m,
    ST_X(ST_Transform(ST_StartPoint(geom), 4326)) AS from_lon,
    ST_Y(ST_Transform(ST_StartPoint(geom), 4326)) AS from_lat,
    ST_X(ST_Transform(ST_EndPoint(geom),   4326)) AS to_lon,
    ST_Y(ST_Transform(ST_EndPoint(geom),   4326)) AS to_lat,
    ST_AsGeoJSON(ST_Transform(geom, 4326)) AS geom_json
  FROM paths.edge
  WHERE fkb_type = 'sti'
    AND length_m BETWEEN 1000 AND 10000
    AND deleted_at IS NULL
    AND ST_Y(ST_Transform(ST_Centroid(geom), 4326)) BETWEEN %(lat_lo)s AND %(lat_hi)s
)
SELECT * FROM cands
ORDER BY md5(id::text)
LIMIT %(n)s
"""

def run_query(lat_lo, lat_hi, n):
    """Bind params manually because psql -c doesn't do %(...)s."""
    sql = QUERY.replace("%(lat_lo)s", str(lat_lo)) \
               .replace("%(lat_hi)s", str(lat_hi)) \
               .replace("%(n)s",      str(n))
    # `-At -F$'\\x1f'` = unaligned tuples-only with a US-separator
    # field delimiter; safer than COPY for nested JSON output.
    wrapped = "SELECT row_to_json(t) FROM (" + sql + ") t"
    out = subprocess.run([PSQL, DB, "-At", "-c", wrapped], capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(f"psql failed: {out.stderr}")
    return [json.loads(line) for line in out.stdout.strip().splitlines() if line.strip()]

def main():
    rows = []
    for region, lat_lo, lat_hi in LAT_BANDS:
        cands = run_query(lat_lo, lat_hi, N_PER_REGION)
        for r in cands:
            geom = json.loads(r["geom_json"])
            coords = geom["coordinates"]
            if len(coords) < 5:
                continue  # too few vertices to be a meaningful polyline
            rows.append({
                "id": r["id"],
                "region": region,
                "length_m": round(r["length_m"], 1),
                "from": [round(r["from_lon"], 6), round(r["from_lat"], 6)],
                "to":   [round(r["to_lon"],   6), round(r["to_lat"],   6)],
                "polyline": [[round(c[0], 6), round(c[1], 6)] for c in coords],
            })
        print(f"  {region:>10s}: {len(cands)} candidates")

    print(f"\nTotal sampled: {len(rows)}")
    # Write TOML.
    out = ["# Terrain-routing ground-truth corpus.",
           "# Auto-generated from paths.edge by tools/sample_terrain_corpus.py.",
           "# Each [[hike]] is a real human-walked sti polyline from the FKB-Veg ingest.",
           "# The polyline IS the ground truth: a smart-terrain solver should",
           "# re-derive these same routing decisions from DEM + landcover inputs.",
           ""]
    for r in rows:
        out.append("[[hike]]")
        out.append(f'id = {r["id"]}')
        out.append(f'region = "{r["region"]}"')
        out.append(f'length_m = {r["length_m"]}')
        out.append(f'from = {r["from"]}')
        out.append(f'to = {r["to"]}')
        out.append("polyline = [")
        for p in r["polyline"]:
            out.append(f"  {p},")
        out.append("]")
        out.append("")
    OUT.write_text("\n".join(out))
    print(f"Wrote {OUT}")

if __name__ == "__main__":
    main()
