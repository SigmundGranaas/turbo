-- terrain.*: vector features extracted from rasters (DTM10, landcover).
-- Rasters live in paths.dem and are read only by ingest jobs that
-- populate these tables. Runtime queries never sample raster.
--
-- All geometry in EPSG:25833 to match paths.* and keep distance
-- computations metric without per-query transforms.

CREATE SCHEMA IF NOT EXISTS terrain;

-- Ridgelines — linear features where the surrounding cells drain
-- AWAY in both perpendicular directions. Ingest computes them from
-- a D8 flow-accumulation pass on the DTM and skeletonises.
CREATE TABLE terrain.ridgeline (
    id              bigserial PRIMARY KEY,
    geom            geometry(LineString, 25833) NOT NULL,
    length_m        double precision GENERATED ALWAYS AS (ST_Length(geom)) STORED,
    mean_elev_m     double precision,
    min_elev_m      double precision,
    max_elev_m      double precision,
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ridgeline_geom_gix ON terrain.ridgeline USING GIST (geom);

-- Drainages — linear features following flow accumulation peaks.
CREATE TABLE terrain.drainage (
    id              bigserial PRIMARY KEY,
    geom            geometry(LineString, 25833) NOT NULL,
    length_m        double precision GENERATED ALWAYS AS (ST_Length(geom)) STORED,
    stream_order    integer,
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX drainage_geom_gix ON terrain.drainage USING GIST (geom);

-- Saddles — points of locally-minimal elevation along ridgelines.
-- Identified by discrete Morse theory persistence at ingest.
CREATE TABLE terrain.saddle (
    id              bigserial PRIMARY KEY,
    geom            geometry(Point, 25833) NOT NULL,
    elev_m          double precision NOT NULL,
    persistence_m   double precision,
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX saddle_geom_gix ON terrain.saddle USING GIST (geom);

-- Landform patches — polygons of contiguous slope/curvature class.
-- One row per region-grown patch; class is the dominant landform.
CREATE TABLE terrain.landform_patch (
    id              bigserial PRIMARY KEY,
    geom            geometry(MultiPolygon, 25833) NOT NULL,
    class           text NOT NULL,                  -- flat|gentle|moderate|steep|cliff
    mean_slope_deg  double precision,
    max_slope_deg   double precision,
    mean_aspect_deg double precision,
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX landform_patch_geom_gix  ON terrain.landform_patch USING GIST (geom);
CREATE INDEX landform_patch_class_idx ON terrain.landform_patch (class);

-- Landcover patches — vectorised from AR5/N50 (already vector at
-- source; we just normalise the class vocabulary).
CREATE TABLE terrain.landcover_patch (
    id              bigserial PRIMARY KEY,
    geom            geometry(MultiPolygon, 25833) NOT NULL,
    class           text NOT NULL,                  -- forest|open|wetland|scree|bare_rock|snow|water|urban
    source          text NOT NULL,                  -- 'ar5' | 'n50' | ...
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX landcover_patch_geom_gix  ON terrain.landcover_patch USING GIST (geom);
CREATE INDEX landcover_patch_class_idx ON terrain.landcover_patch (class);

-- Water bodies — lakes, fjords. Separate from water courses (streams).
CREATE TABLE terrain.water_polygon (
    id              bigserial PRIMARY KEY,
    geom            geometry(MultiPolygon, 25833) NOT NULL,
    name            text,
    kind            text NOT NULL,                  -- lake|fjord|reservoir
    elev_m          double precision,
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX water_polygon_geom_gix ON terrain.water_polygon USING GIST (geom);

-- Treeline — vector boundary between forest and open above it.
CREATE TABLE terrain.treeline (
    id              bigserial PRIMARY KEY,
    geom            geometry(LineString, 25833) NOT NULL,
    mean_elev_m     double precision,
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX treeline_geom_gix ON terrain.treeline USING GIST (geom);

-- Glaciers — refused region for summer hiking, allowed for ski.
CREATE TABLE terrain.glacier_polygon (
    id              bigserial PRIMARY KEY,
    geom            geometry(MultiPolygon, 25833) NOT NULL,
    name            text,
    attrs           jsonb NOT NULL DEFAULT '{}'::jsonb,
    ingested_at     timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX glacier_polygon_geom_gix ON terrain.glacier_polygon USING GIST (geom);
