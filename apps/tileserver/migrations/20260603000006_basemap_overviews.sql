-- Low-zoom overview materialized views for the basemap.
--
-- At z4–11 a single tile covers a huge area, so rendering it straight from
-- terrain.*/paths.edge means ST_AsMVTGeom + per-feature
-- ST_SimplifyPreserveTopology over millions of national rows on every
-- request. These matviews pre-simplify the geometry and drop sub-pixel
-- features once (at provision/refresh time), so a low-zoom tile reads a far
-- smaller, already-generalised set and just clips it.
--
-- Each view exposes exactly the columns the matching basemap-layers.toml
-- layer reads (geom + its attrs), and bakes in that layer's filters, so the
-- render path can point at the view and skip both simplify and filter.
-- Tolerances are sized for the TOP of each layer's overview zoom band so
-- nothing is visibly over-simplified there. Refreshed by the
-- `refresh-basemap-overviews` ingest job (and at the end of provision-n50).

CREATE SCHEMA IF NOT EXISTS basemap;

-- Water: drop tarns < ~0.05 km² (sub-pixel below z9), simplify to ~90 m.
CREATE MATERIALIZED VIEW basemap.water_overview AS
    SELECT ST_SimplifyPreserveTopology(geom, 90) AS geom, kind
    FROM terrain.water_polygon
    WHERE source = 'n50' AND ST_Area(geom) > 50000;
CREATE INDEX water_overview_gix ON basemap.water_overview USING GIST (geom);

-- Landcover: drop patches < 0.1 km², simplify to ~90 m.
CREATE MATERIALIZED VIEW basemap.landcover_overview AS
    SELECT ST_SimplifyPreserveTopology(geom, 90) AS geom, class
    FROM terrain.landcover_patch
    WHERE source = 'n50' AND ST_Area(geom) > 100000;
CREATE INDEX landcover_overview_gix ON basemap.landcover_overview USING GIST (geom);

-- Coastline: drop slivers < 150 m, simplify to ~90 m.
CREATE MATERIALIZED VIEW basemap.coastline_overview AS
    SELECT ST_SimplifyPreserveTopology(geom, 90) AS geom
    FROM terrain.coastline
    WHERE source = 'n50' AND ST_Length(geom) > 150;
CREATE INDEX coastline_overview_gix ON basemap.coastline_overview USING GIST (geom);

-- Transportation: roads only (drop sti/trails at low zoom), simplify to ~20 m.
-- fkb_type + marking are the columns the `transportation` layer reads.
CREATE MATERIALIZED VIEW basemap.transportation_overview AS
    SELECT ST_SimplifyPreserveTopology(geom, 20) AS geom, fkb_type, marking
    FROM paths.edge
    WHERE deleted_at IS NULL
      AND fkb_type IN ('vei', 'traktorvei', 'skogsvei', 'sykkelvei');
CREATE INDEX transportation_overview_gix ON basemap.transportation_overview USING GIST (geom);

-- Contour: index lines (every 100 m) only, simplify to ~30 m.
CREATE MATERIALIZED VIEW basemap.contour_overview AS
    SELECT ST_SimplifyPreserveTopology(geom, 30) AS geom, elev_m, is_index, kind
    FROM terrain.contour
    WHERE source = 'n50' AND is_index;
CREATE INDEX contour_overview_gix ON basemap.contour_overview USING GIST (geom);
