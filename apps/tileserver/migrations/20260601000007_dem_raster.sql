-- DTM raster storage. Enables postgis_raster (separate extension from
-- core postgis) and creates a `paths.dem` table that holds Kartverket
-- DTM10 / DTM1 GeoTIFF tiles.
--
-- The `dtm-load` ingest job (turbo-tiles-ingest::dtm_raster) shells
-- out to raster2pgsql to populate this table from .tif files dropped
-- on the configured incoming volume; `dtm10-attach` then samples
-- ST_Value(rast, vertex) per edge.

CREATE EXTENSION IF NOT EXISTS postgis_raster;

CREATE TABLE IF NOT EXISTS paths.dem (
    rid     serial PRIMARY KEY,
    rast    raster NOT NULL,
    -- raster2pgsql doesn't write this column; load_geotiff stamps it
    -- after the import. Default '' so the INSERTs raster2pgsql emits
    -- still satisfy NOT NULL.
    source  text   NOT NULL DEFAULT '',
    loaded_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS dem_st_convexhull_gix
    ON paths.dem USING GIST (ST_ConvexHull(rast));
CREATE INDEX IF NOT EXISTS dem_source_idx ON paths.dem (source);

-- Helper: returns true if any DEM tile covers the given point in 25833.
-- The attach job uses this to decide between local raster sampling
-- (fast) and the live Hoydedata service (per-vertex HTTPS call).
CREATE OR REPLACE FUNCTION paths.dem_covers(p geometry) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT EXISTS (
        SELECT 1 FROM paths.dem
        WHERE ST_Intersects(ST_ConvexHull(rast), p)
        LIMIT 1
    );
$$;
