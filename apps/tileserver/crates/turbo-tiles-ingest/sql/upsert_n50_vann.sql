-- Upsert N50 lakes (innsjo) + regulated lakes/reservoirs (innsjoregulert)
-- + sea (havflate) → terrain.water_polygon.
-- Sources after pgdump_load::restore: schema `n50_staging`.
--
-- Source table shape (from the dump):
--   n50_staging.innsjo (objid, objtype, omrade GEOMETRY(_,25833),
--                       vatnlopenummer, hoyde, oppdateringsdato)
--   n50_staging.innsjoregulert (objid, objtype, omrade GEOMETRY(_,25833),
--                       vatnlopenummer, lavesteregulertevannstand, hoyde,
--                       oppdateringsdato)
--   n50_staging.havflate (objid, objtype, omrade GEOMETRY(_,25833),
--                         oppdateringsdato)
--
-- Geometry column is `omrade` (Norwegian for "area"), not `geom`.
--
-- NOTE: `innsjoregulert` (regulated lakes/reservoirs — the "128-115"
-- level-range lakes on the map) was previously NOT ingested, so all ~1500
-- regulated water bodies in Norway were absent from the water mask and the
-- off-trail solver routed straight across them (e.g. Heggmovatnet). They are
-- just as impassable as natural lakes, so they go into water_polygon too.
--
-- SUBDIVISION: every polygon is stored as `ST_Subdivide(..., 256)` chunks
-- rather than one giant multipolygon. The national `havflate` (sea) is a
-- multi-million-vertex polygon; without this the basemap MVT render had to
-- load + clip the entire sea for EVERY coastal tile (a ~14s, multi-hundred-MB
-- query that OOM-killed the 2Gi pod). Subdividing into ≤256-vertex chunks
-- (each GIST-indexed) means a tile query only touches the handful of chunks it
-- overlaps. It's a no-op for the many small lakes (already < 256 vertices) and
-- coverage-preserving, so the water mask / routing are unaffected — only the
-- row count grows. Pairs with the render-side clip-before-simplify.

DELETE FROM terrain.water_polygon WHERE source = 'n50';

INSERT INTO terrain.water_polygon (geom, name, kind, elev_m, source, attrs)
SELECT
    ST_Multi(chunk),
    NULL::text,
    'lake'::text,
    s.hoyde::float8,
    'n50',
    jsonb_build_object('vatnlopenummer', s.vatnlopenummer, 'objtype', s.objtype)
FROM n50_staging.innsjo s,
     LATERAL ST_Subdivide(ST_CollectionExtract(ST_MakeValid(s.omrade), 3), 256) AS chunk
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);

-- Regulated lakes / reservoirs — just as impassable as natural lakes.
INSERT INTO terrain.water_polygon (geom, name, kind, elev_m, source, attrs)
SELECT
    ST_Multi(chunk),
    NULL::text,
    'lake'::text,
    s.hoyde::float8,
    'n50',
    jsonb_build_object(
        'vatnlopenummer', s.vatnlopenummer,
        'objtype', s.objtype,
        'lavesteregulertevannstand', s.lavesteregulertevannstand
    )
FROM n50_staging.innsjoregulert s,
     LATERAL ST_Subdivide(ST_CollectionExtract(ST_MakeValid(s.omrade), 3), 256) AS chunk
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);

-- River-area polygons (`elv`) — rivers wide enough that N50 maps them as
-- AREAS, not centrelines. ~22k features / ~1150 km² nationally. These were
-- NEVER ingested (water_polygon held only lake + sea), so the solver crossed
-- big rivers for free — the exact same data gap as the regulated lakes. A
-- mapped river area is impassable on foot; it belongs in water_polygon.
INSERT INTO terrain.water_polygon (geom, name, kind, source, attrs)
SELECT
    ST_Multi(chunk),
    NULL::text,
    'river'::text,
    'n50',
    jsonb_build_object('objtype', s.objtype)
FROM n50_staging.elv s,
     LATERAL ST_Subdivide(ST_CollectionExtract(ST_MakeValid(s.omrade), 3), 256) AS chunk
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);

-- Intermittent / dry-fall freshwater (`ferskvanntorrfall`) — riverbeds and
-- lake margins that are usually wet. ~2.8k features / ~42 km². Passable when
-- dry but normally water; treated as (lighter) water so routes don't cut
-- straight through a braided riverbed.
INSERT INTO terrain.water_polygon (geom, name, kind, source, attrs)
SELECT
    ST_Multi(chunk),
    NULL::text,
    'river_dry'::text,
    'n50',
    jsonb_build_object('objtype', s.objtype)
FROM n50_staging.ferskvanntorrfall s,
     LATERAL ST_Subdivide(ST_CollectionExtract(ST_MakeValid(s.omrade), 3), 256) AS chunk
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);

INSERT INTO terrain.water_polygon (geom, name, kind, source, attrs)
SELECT
    ST_Multi(chunk),
    NULL::text,
    'sea'::text,
    'n50',
    jsonb_build_object('objtype', s.objtype)
FROM n50_staging.havflate s,
     LATERAL ST_Subdivide(ST_CollectionExtract(ST_MakeValid(s.omrade), 3), 256) AS chunk
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);
