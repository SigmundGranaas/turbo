#!/usr/bin/env bash
# Pull a single N50 Kartdata county (fylke) from the Geonorge Nedlasting API
# as a PostGIS dump — the small, fast sample we use for local dev and tests.
#
# Why per-fylke: the whole-country N50 PostGIS dump is ~25 GB. A single
# county (Oslo `03` is the smallest, ~8 MB zipped) exercises every ingest
# code path against REAL Kartverket data in seconds. Swap the code for a
# bigger/mountainous county when you need richer contours/glaciers.
#
# Usage:
#   scripts/pull-n50-fylke.sh [FYLKE_CODE] [OUT_DIR]
#   scripts/pull-n50-fylke.sh 03 /tmp/n50oslo      # Oslo (default)
#   scripts/pull-n50-fylke.sh 46 /tmp/n50vestland  # Vestland (fjords+glaciers)
#
# Fylke codes: 03 Oslo, 11 Rogaland, 15 Møre og Romsdal, 18 Nordland,
#              31 Østfold, 32 Akershus, 33 Buskerud, 34 Innlandet,
#              39 Vestfold, 40 Telemark, 42 Agder, 46 Vestland,
#              50 Trøndelag, 55 Troms, 56 Finnmark.
set -euo pipefail

FYLKE="${1:-03}"
OUT="${2:-/tmp/n50_${FYLKE}}"
UUID="ea192681-d039-42ec-b1bc-f3ce04c189ac"   # N50 Kartdata metadata UUID
API="https://nedlasting.geonorge.no/api"

mkdir -p "$OUT"
echo "Ordering N50 Kartdata for fylke $FYLKE (PostGIS / EUREF89 UTM33)…"

ORDER=$(curl -fsS -X POST "$API/order" \
  -H "Content-Type: application/json" -H "Accept: application/json" \
  -d "{
    \"email\": \"noreply@example.com\",
    \"softwareClient\": \"turbo-ingest\", \"softwareClientVersion\": \"1.0\",
    \"orderLines\": [{
      \"metadataUuid\": \"$UUID\",
      \"areas\":       [{ \"code\": \"$FYLKE\", \"type\": \"fylke\" }],
      \"formats\":     [{ \"name\": \"PostGIS\" }],
      \"projections\": [{ \"code\": \"25833\",
                          \"codespace\": \"http://www.opengis.net/def/crs/EPSG/0/25833\" }]
    }]
  }")

URL=$(printf '%s' "$ORDER"  | grep -oE '"downloadUrl":"[^"]+"' | head -1 | cut -d'"' -f4)
NAME=$(printf '%s' "$ORDER" | grep -oE '"name":"[^"]+"'        | head -1 | cut -d'"' -f4)
[ -n "$URL" ] || { echo "no download URL in order response:"; echo "$ORDER"; exit 1; }

echo "Downloading $NAME …"
curl -fsS -L -o "$OUT/n50.zip" "$URL"
( cd "$OUT" && unzip -o n50.zip >/dev/null )
echo "Done. Dump at: $OUT/n50.zip"
echo
echo "Next:"
echo "  export DATABASE_URL=postgres://postgres:testpass@localhost:5432/tiles"
echo "  tileserver migrate"
echo "  tileserver ingest --job n50-restore --file $OUT/n50.zip"
echo "  for j in vann hoydekurve isogbre landcover stedsnavn vegnett; do \\"
echo "    tileserver ingest --job n50-\$j-upsert; done"
