-- Realistic fixture: a grid of edges + nodes around Nordmarka (north
-- of Oslo) so MVT, routing, and admin-panel queries have real data to
-- chew on when the Geonorge WFS isn't reachable.
--
-- Geometry: 10×10 grid of nodes, spaced 200 m apart in EPSG:25833.
-- Edges connect horizontal and vertical neighbours, giving us a
-- connected graph for pgr_dijkstra. Centre point ~ Sognsvann.
--
-- Variety:
--   - All edges are 'sti' or 'traktorveg' (so v_hiking_trails is
--     populated)
--   - Half marked, half unmarked
--   - A few edges are 'skiloype' so v_ski_tracks is non-empty
--   - One published curated_route ties three edges together

TRUNCATE paths.edge, paths.node, paths.curated_route, paths.ingest_job RESTART IDENTITY CASCADE;

WITH params AS (
  -- Sognsvann area, EPSG:25833 (UTM33N).
  SELECT 595000.0 AS x0, 6650000.0 AS y0, 200.0 AS step, 10 AS n
),
nodes AS (
  INSERT INTO paths.node (geom)
  SELECT ST_SetSRID(
           ST_Point(p.x0 + i * p.step, p.y0 + j * p.step),
           25833
         )
  FROM params p, generate_series(0, 9) i, generate_series(0, 9) j
  ORDER BY j, i
  RETURNING id, geom
),
indexed_nodes AS (
  SELECT id, geom, ROW_NUMBER() OVER (ORDER BY ST_Y(geom), ST_X(geom)) - 1 AS rn
  FROM nodes
)
SELECT 'seeded ' || COUNT(*) || ' nodes' FROM indexed_nodes;

-- Build edges from the seeded nodes by reading them back. We need to
-- carry the row index forward to address neighbours, so a CTE drives
-- the inserts.
WITH params AS (
  SELECT 200.0 AS step, 10 AS n
),
g AS (
  SELECT n.id,
         (ROW_NUMBER() OVER (ORDER BY n.id) - 1) AS rn
  FROM paths.node n
),
pairs AS (
  SELECT a.id AS a_id, b.id AS b_id, a.rn AS arn, b.rn AS brn
  FROM g a
  JOIN g b
    ON (b.rn = a.rn + 1 AND a.rn % 10 < 9)         -- horizontal neighbour
    OR (b.rn = a.rn + 10 AND a.rn < 90)            -- vertical neighbour
)
INSERT INTO paths.edge
  (source_node, target_node, geom, fkb_type, marking, surface, attr_hash, ingest_source, season)
SELECT
  p.a_id,
  p.b_id,
  ST_MakeLine(na.geom, nb.geom),
  CASE
    WHEN p.arn % 23 = 0 THEN 'skiloype'
    WHEN p.arn % 5 = 0 THEN 'traktorveg'
    ELSE 'sti'
  END,
  CASE WHEN p.arn % 2 = 0 THEN 'red' ELSE NULL END,
  CASE WHEN p.arn % 3 = 0 THEN 'gravel' ELSE 'dirt' END,
  encode(sha256(('seed-' || p.a_id || '-' || p.b_id)::bytea), 'hex'),
  'fkb',
  ARRAY['summer']::text[]
FROM pairs p
JOIN paths.node na ON na.id = p.a_id
JOIN paths.node nb ON nb.id = p.b_id;

-- One published curated route stringing three edges together — makes
-- the hiking-trails view non-trivial.
INSERT INTO paths.curated_route
  (resource, slug, name, description, difficulty, marking, season, surface, edge_ids, geom, source, status, attribution)
SELECT
  'hiking-trails',
  'sognsvann-loop',
  'Sognsvann ridge loop',
  'A short marked loop seeded for end-to-end testing.',
  'easy',
  'red',
  ARRAY['summer']::text[],
  'gravel',
  ARRAY[]::bigint[],
  ST_Multi(ST_LineMerge(ST_Collect(e.geom))),
  'manual',
  'published',
  '© Test fixture'
FROM (
  SELECT geom FROM paths.edge ORDER BY id LIMIT 3
) e;

-- A second route in draft state so the admin list shows mixed status.
INSERT INTO paths.curated_route
  (resource, slug, name, difficulty, status, geom, source, season)
SELECT
  'hiking-trails',
  'maridalsalpene-traverse',
  'Maridalsalpene traverse',
  'hard',
  'draft',
  ST_Multi(geom),
  'manual',
  ARRAY['summer']::text[]
FROM paths.edge ORDER BY id DESC LIMIT 1;

SELECT 'edges' AS kind, COUNT(*) FROM paths.edge
UNION ALL SELECT 'nodes', COUNT(*) FROM paths.node
UNION ALL SELECT 'curated_routes', COUNT(*) FROM paths.curated_route;
