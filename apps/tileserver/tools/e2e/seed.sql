-- Deterministic synthetic data for the primitives E2E.
--
-- Geography: an area centred on Oslo (~262_000, 6_649_000 in EPSG:25833).
-- Two adjacent 256×256 DTM-style rasters (10 m pixels), a 5×5 routing
-- node lattice, 12 directed edges, a small lake polygon, and a handful
-- of named anchors. Everything is small enough to seed in <1 s and big
-- enough that every primitive returns a non-trivial answer.

\set ON_ERROR_STOP on

-- Coordinate system anchors (UTM33N metres). Pick origin on a clean
-- 10 m grid + 256-cell tile boundary so raster2pgsql alignment math
-- is trivial to reason about.
--
--   tile A upperleft = (260_000, 6_652_560), elev ≡ 500 m
--   tile B upperleft = (262_560, 6_652_560), elev ≡ 550 m  (slope step on boundary)
--   merged extent    = x ∈ [260_000, 265_120],
--                      y ∈ [6_650_000, 6_652_560]
--   merged grid      = 512 × 256 cells

TRUNCATE paths.dem RESTART IDENTITY CASCADE;
TRUNCATE paths.edge RESTART IDENTITY CASCADE;
TRUNCATE paths.node RESTART IDENTITY CASCADE;
TRUNCATE terrain.water_polygon RESTART IDENTITY CASCADE;
TRUNCATE terrain.glacier_polygon RESTART IDENTITY CASCADE;
TRUNCATE anchors.anchor RESTART IDENTITY CASCADE;

-- --- DEM rasters ---------------------------------------------------------
--
-- Two 256×256 tiles, each a constant elevation. The DEM builder doesn't
-- care about per-pixel variation as long as the tile shape matches
-- (raster2pgsql -t 256x256 default).

INSERT INTO paths.dem (rast, source)
SELECT
    ST_AddBand(
        ST_MakeEmptyRaster(256, 256, 260000, 6652560, 10.0, -10.0, 0, 0, 25833),
        '32BF'::text,
        500.0,
        -9999.0
    ),
    'e2e_tile_a';

INSERT INTO paths.dem (rast, source)
SELECT
    ST_AddBand(
        ST_MakeEmptyRaster(256, 256, 262560, 6652560, 10.0, -10.0, 0, 0, 25833),
        '32BF'::text,
        550.0,
        -9999.0
    ),
    'e2e_tile_b';

-- --- Graph: 5×5 node lattice in tile A, ~500 m spacing ------------------
--
-- Grid layout (row 0 = north):
--   00 01 02 03 04
--   05 06 07 08 09
--   10 11 12 13 14
--   15 16 17 18 19
--   20 21 22 23 24
--
-- Origin at (260_500, 6_652_000). Step 500 m east, 500 m south.

WITH grid AS (
    SELECT
        r * 5 + c                                     AS i,
        ST_SetSRID(ST_MakePoint(
            260500.0 + c * 500.0,
            6652000.0 - r * 500.0
        ), 25833)                                     AS geom
    FROM generate_series(0, 4) r, generate_series(0, 4) c
)
INSERT INTO paths.node (id, geom)
SELECT i + 1, geom FROM grid ORDER BY i;

SELECT setval(pg_get_serial_sequence('paths.node', 'id'), (SELECT MAX(id) FROM paths.node));

-- Edges: 4-connectivity (east + south neighbours), bidirectional via the
-- builder which materialises reverse edges. fkb sti for the centre row,
-- dnt for the verticals so the PreferredEdgeLayer can favour them.
INSERT INTO paths.edge
    (source_node, target_node, geom, fkb_type, marking, surface, attrs, attr_hash, ingest_source)
SELECT
    a.id, b.id,
    ST_MakeLine(a.geom, b.geom),
    'sti',
    CASE WHEN a.id <= 5 THEN 'red_t' ELSE NULL END,
    'natural',
    '{}'::jsonb,
    'e2e_h_' || a.id || '_' || b.id,
    'fkb'::paths.ingest_source
FROM paths.node a, paths.node b
WHERE b.id = a.id + 1 AND a.id % 5 != 0;  -- horizontal east neighbours

INSERT INTO paths.edge
    (source_node, target_node, geom, fkb_type, marking, surface, attrs, attr_hash, ingest_source)
SELECT
    a.id, b.id,
    ST_MakeLine(a.geom, b.geom),
    'sti',
    'red_t',
    'natural',
    '{}'::jsonb,
    'e2e_v_' || a.id || '_' || b.id,
    'dnt'::paths.ingest_source
FROM paths.node a, paths.node b
WHERE b.id = a.id + 5 AND a.id <= 20;  -- vertical south neighbours

-- --- Water polygon: a small lake in the northeast corner of tile A ------
--
-- 200 m square centred at (262_200, 6_652_200). Sits between the graph
-- nodes so a path that's allowed to ignore refusal can cut through it.

INSERT INTO terrain.water_polygon (geom, name, kind)
VALUES (
    ST_Multi(ST_SetSRID(ST_MakeEnvelope(262100, 6652100, 262300, 6652300), 25833)),
    'E2E Lake',
    'lake'
);

-- --- Anchors: 3 summits, 2 cabins, 1 waterfeature -----------------------

INSERT INTO anchors.anchor (kind, geom, name, elevation_m, source_ref)
VALUES
    ('summit',       ST_SetSRID(ST_MakePoint(261000, 6651800), 25833), 'Galdhøpiggen Test',  1500, 'e2e_summit_1'),
    ('summit',       ST_SetSRID(ST_MakePoint(263200, 6651000), 25833), 'Trolltunga Test',    1200, 'e2e_summit_2'),
    ('summit',       ST_SetSRID(ST_MakePoint(264000, 6650500), 25833), 'Preikestolen Test',  1000, 'e2e_summit_3'),
    ('cabin',        ST_SetSRID(ST_MakePoint(261500, 6651500), 25833), 'Test Cabin North',     800, 'e2e_cabin_1'),
    ('cabin',        ST_SetSRID(ST_MakePoint(263500, 6650800), 25833), 'Test Cabin South',     750, 'e2e_cabin_2'),
    ('waterfeature', ST_SetSRID(ST_MakePoint(262200, 6652200), 25833), 'Test Lake',            450, 'e2e_water_1')
;

-- --- Sanity counts (printed at e2e runtime) -----------------------------
SELECT 'paths.dem' AS table, COUNT(*) FROM paths.dem
UNION ALL SELECT 'paths.node', COUNT(*) FROM paths.node
UNION ALL SELECT 'paths.edge', COUNT(*) FROM paths.edge
UNION ALL SELECT 'terrain.water_polygon', COUNT(*) FROM terrain.water_polygon
UNION ALL SELECT 'anchors.anchor', COUNT(*) FROM anchors.anchor;
