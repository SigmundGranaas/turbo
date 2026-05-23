#!/usr/bin/env python3
"""Generate a synthetic DTM10 GeoTIFF over the seeded fixture area.

Real Kartverket DTM10 tiles are 6 GB nationwide; this script writes a
tiny GeoTIFF (~few hundred KB) so the dtm-load → dtm10-attach pipeline
can be exercised end-to-end without an external dataset. Elevation is
a smooth gradient + sinusoid so per-edge gain/loss values are
non-trivial when sampled.

Output: a single-band Float32 GeoTIFF in EPSG:25833 covering the
Sognsvann-area grid (x: 594900..597100, y: 6649900..6651100).
Run: `python3 tools/synth-dtm.py > /tmp/dtm10-synth.tif`.
"""

from osgeo import gdal, osr
import math
import sys

# EPSG:25833 extent that wraps the seeded grid with a small margin.
X0, Y0 = 594900.0, 6649900.0
X1, Y1 = 597100.0, 6651100.0
RES = 10.0  # 10 m DTM10 resolution

width = int((X1 - X0) / RES)
height = int((Y1 - Y0) / RES)

# Use MEM driver then CreateCopy to GTiff so we don't need to handle
# bytes directly on stdout — gdal_translate-style.
mem = gdal.GetDriverByName("MEM").Create("", width, height, 1, gdal.GDT_Float32)
mem.SetGeoTransform([X0, RES, 0, Y1, 0, -RES])
srs = osr.SpatialReference()
srs.ImportFromEPSG(25833)
mem.SetProjection(srs.ExportToWkt())

band = mem.GetRasterBand(1)
buf = bytearray()
import struct

for j in range(height):
    y = Y1 - (j + 0.5) * RES
    row = []
    for i in range(width):
        x = X0 + (i + 0.5) * RES
        # 200 m baseline + 30 m sinusoidal variation + 15 m gradient
        elev = (
            200.0
            + 30.0 * math.sin((x - X0) / 600.0) * math.cos((y - Y0) / 400.0)
            + (x - X0) * 15.0 / (X1 - X0)
        )
        row.append(elev)
    band.WriteRaster(0, j, width, 1, struct.pack(f"{width}f", *row))

band.FlushCache()

dst_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/dtm10-synth.tif"
gdal.GetDriverByName("GTiff").CreateCopy(
    dst_path,
    mem,
    options=["COMPRESS=DEFLATE", "TILED=YES"],
)
print(f"wrote {dst_path} ({width}x{height}, EPSG:25833)")
