-- Seed fixture for the recommendation pipeline. Populates anchors,
-- terrain features, and one trail so the /v1/recommend/* endpoints
-- return something meaningful in dev without needing Geonorge.
--
-- Assumes the Oslomarka graph fixture has already been loaded
-- (`tools/seed-oslo-fixture.sql`). This script attaches anchors and
-- terrain features tied to the geometry in that fixture.
--
-- Run via `tileserver ingest --job recommend-seed` or as a one-off
-- SQL execution; both paths share this script.

-- 0. Self-contained dev fixture: load the Oslomarka grid (nodes +
-- edges) so the rest of this fixture has graph nodes to snap anchors
-- to.
--
-- SAFETY: refuse to run if real ingest data is present. The fixture
-- TRUNCATEs paths.node — which would destroy real Turrutebasen or
-- FKB topology. The check below raises an explicit exception when
-- any real-data edges exist, so the curator must run a reset before
-- re-seeding.
DO $$
DECLARE
    real_edges bigint;
BEGIN
    SELECT COUNT(*) INTO real_edges
    FROM paths.edge
    WHERE deleted_at IS NULL
      AND ingest_source IN ('turbase', 'fkb', 'dnt');
    IF real_edges > 0 THEN
        RAISE EXCEPTION 'recommend-seed refuses to run: % real-ingest edges present in paths.edge. '
                        'Run reset/all from the admin panel first, or DELETE FROM paths.edge '
                        'WHERE ingest_source IN (''turbase'',''fkb'',''dnt'') manually.', real_edges;
    END IF;
END $$;

DELETE FROM paths.edge WHERE ingest_source IN ('manual', 'skeleton');
TRUNCATE paths.node RESTART IDENTITY CASCADE;

WITH params AS (
    SELECT 595000.0::float8 AS x0, 6650000.0::float8 AS y0,
           200.0::float8 AS step
)
INSERT INTO paths.node (geom)
SELECT ST_SetSRID(ST_Point(p.x0 + i * p.step, p.y0 + j * p.step), 25833)
FROM params p, generate_series(0, 9) i, generate_series(0, 9) j
ORDER BY j, i;

-- Edges connecting orthogonal grid neighbours.
WITH g AS (
    SELECT n.id, (ROW_NUMBER() OVER (ORDER BY n.id) - 1) AS rn FROM paths.node n
),
pairs AS (
    SELECT a.id AS a_id, b.id AS b_id, a.rn AS arn
    FROM g a JOIN g b
      ON (b.rn = a.rn + 1 AND a.rn % 10 < 9)
      OR (b.rn = a.rn + 10 AND a.rn < 90)
)
INSERT INTO paths.edge
    (source_node, target_node, geom, fkb_type, marking, surface,
     attr_hash, ingest_source, season)
SELECT
    p.a_id, p.b_id,
    ST_MakeLine(na.geom, nb.geom),
    CASE WHEN p.arn % 23 = 0 THEN 'skiloype'
         WHEN p.arn % 5 = 0 THEN 'traktorveg'
         ELSE 'sti' END,
    CASE WHEN p.arn % 2 = 0 THEN 'red' ELSE NULL END,
    CASE WHEN p.arn % 3 = 0 THEN 'gravel' ELSE 'dirt' END,
    encode(sha256(('seed-grid-' || p.a_id || '-' || p.b_id)::bytea), 'hex'),
    'manual'::paths.ingest_source,
    ARRAY['summer']::text[]
FROM pairs p
JOIN paths.node na ON na.id = p.a_id
JOIN paths.node nb ON nb.id = p.b_id;

-- 1. Clear prior seed rows so re-running is idempotent. We only
-- TRUNCATE the namespaces this fixture owns; real-ingest tables are
-- left alone. Note: no synthetic ridgelines/saddles — those must
-- come from authoritative sources, never from raster heuristics.
TRUNCATE anchors.anchor RESTART IDENTITY;
DELETE FROM terrain.water_polygon WHERE attrs->>'source' = 'seed';
DELETE FROM trails.trail        WHERE source = 'manual' AND source_ref LIKE 'seed-%';

-- 2. Plant a handful of anchors centred on the Sognsvann grid. Each
-- gets pre-snapped to the nearest paths.node row. Distances are in
-- metres because we're in EPSG:25833.
--
-- Geometry layout: the Oslo fixture grid starts at (595000, 6650000)
-- with 200 m spacing for a 10×10 grid. We place anchors at known
-- grid positions so the snap is deterministic.
WITH grid AS (
    SELECT 595000.0::float8 AS x0,
           6650000.0::float8 AS y0,
           200.0::float8     AS step
),
anchor_seeds AS (
    SELECT * FROM (VALUES
        ('summit',      'Vettakollen',         9, 9, 481.0, 220.0),
        ('summit',      'Grefsenkollen',       2, 8, 380.0, 180.0),
        ('summit',      'Tryvannshogda',       5, 7, 529.0, 260.0),
        ('viewpoint',   'Frognerseteren',      3, 5, 419.0, NULL),
        ('cabin',       'Kobberhaughytta',     7, 6, 425.0, NULL),
        ('trailhead',   'Sognsvann parkering', 1, 1, 178.0, NULL),
        ('waterfeature','Sognsvann',           1, 3, 178.0, NULL)
    ) AS t(kind, name, gx, gy, elev, prominence)
)
INSERT INTO anchors.anchor
    (kind, geom, name, elevation_m, prominence_m, sources, source_ref, attrs)
SELECT
    s.kind,
    ST_SetSRID(ST_Point(g.x0 + s.gx * g.step, g.y0 + s.gy * g.step), 25833),
    s.name,
    s.elev,
    s.prominence,
    ARRAY['fixture']::text[],
    'seed-' || s.kind || '-' || lower(replace(s.name, ' ', '-')),
    jsonb_build_object('source', 'seed', 'fixture', 'oslomarka')
FROM grid g, anchor_seeds s;

-- 3. Snap every anchor to its nearest paths.node. We use a single
-- lateral subquery so the snap is one round-trip per anchor, not a
-- correlated subquery on each row.
UPDATE anchors.anchor a
SET snapped_node_id = nn.node_id,
    snap_distance_m = nn.dist
FROM (
    SELECT a2.id AS aid,
           n.id AS node_id,
           ST_Distance(n.geom, a2.geom) AS dist,
           ROW_NUMBER() OVER (
               PARTITION BY a2.id ORDER BY ST_Distance(n.geom, a2.geom)
           ) AS rn
    FROM anchors.anchor a2
    JOIN paths.node n ON ST_DWithin(n.geom, a2.geom, 1000.0)
) nn
WHERE nn.aid = a.id AND nn.rn = 1;

-- 4. A small lake polygon around the seeded "Sognsvann" anchor so
-- terrain queries find a water body. This is real-shape stand-in
-- data, clearly marked as a fixture in attrs.source.
INSERT INTO terrain.water_polygon (geom, name, kind, attrs)
SELECT
    ST_Multi(ST_Buffer(a.geom, 150.0)),
    a.name,
    'lake',
    jsonb_build_object('source', 'seed')
FROM anchors.anchor a
WHERE a.kind = 'waterfeature';

-- 5. One marked trail spanning three edges so the trails namespace is
-- exercised end-to-end.
WITH first_three AS (
    SELECT id, ROW_NUMBER() OVER (ORDER BY id) AS seq
    FROM paths.edge ORDER BY id LIMIT 3
),
new_trail AS (
    INSERT INTO trails.trail (name, operator, mark_colour, grade, source, source_ref, attrs)
    VALUES ('Sognsvann ridge loop', 'DNT', 'red', 'bla', 'manual', 'seed-sognsvann-loop',
            jsonb_build_object('source', 'seed'))
    RETURNING id
)
INSERT INTO trails.trail_edge (trail_id, edge_id, seq)
SELECT (SELECT id FROM new_trail), ft.id, ft.seq
FROM first_three ft;

-- Summary so the operator can see what landed.
SELECT 'anchors'        AS kind, COUNT(*) FROM anchors.anchor
UNION ALL SELECT 'water_polygons', COUNT(*) FROM terrain.water_polygon
UNION ALL SELECT 'trails',         COUNT(*) FROM trails.trail;
