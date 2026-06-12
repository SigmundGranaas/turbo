#!/usr/bin/env bash
#
# One command to run the Places API locally with sample data — no network.
#
#   ./apps/api/src/Places/places-dev.sh
#
# Needs: cargo, dotnet (10), docker. Works on Linux + macOS.
# Then, in another terminal:
#   curl 'http://localhost:5179/api/places/reverse?lat=61.6363&lon=8.3120'
#   curl 'http://localhost:5179/api/places/search?q=galdh&lat=61.6363&lon=8.3120'
#   curl 'http://localhost:5179/api/places/bundle?bbox=8.0,61.4,8.6,61.8' -o region.sqlite
#
set -euo pipefail

API_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"   # apps/api
ROOT="$(cd "$API_DIR/../.." && pwd)"                            # repo root
CORE="$ROOT/packages/place-core"
PORT="${PLACES_PORT:-5179}"
DB_CONTAINER="turbo-places-db"
DB_PORT="${PLACES_DB_PORT:-55432}"
CONN="Host=localhost;Port=$DB_PORT;Database=places;Username=postgres;Password=places"

echo "==> 1/4 building place-core native lib (cabi,embedded)"
( cd "$CORE" && cargo build --features cabi,embedded )
export PLACE_CORE_LIB="$CORE/target/debug"

echo "==> 2/4 starting PostGIS ($DB_CONTAINER on :$DB_PORT)"
if ! docker ps --format '{{.Names}}' | grep -qx "$DB_CONTAINER"; then
  docker rm -f "$DB_CONTAINER" >/dev/null 2>&1 || true
  docker run -d --name "$DB_CONTAINER" \
    -e POSTGRES_PASSWORD=places -e POSTGRES_DB=places -p "$DB_PORT:5432" \
    postgis/postgis:16-3.4 >/dev/null
fi
echo -n "    waiting for postgres"
for _ in $(seq 1 60); do
  if docker exec "$DB_CONTAINER" pg_isready -U postgres >/dev/null 2>&1; then echo " ready"; break; fi
  echo -n "."; sleep 1
done

echo "==> 3/4 seeding sample data (offline)"
PLACES_DB="$CONN" dotnet run --project "$API_DIR/src/Places/Turbo.Places.Ingestion" -- seed-samples

echo "==> 4/4 starting Places API on http://localhost:$PORT"
echo "    try:  curl 'http://localhost:$PORT/api/places/reverse?lat=61.6363&lon=8.3120'"
export ConnectionStrings__Places="$CONN"
export ASPNETCORE_URLS="http://localhost:$PORT"
exec dotnet run --project "$API_DIR/hosts/Turbo.Host.Places"
