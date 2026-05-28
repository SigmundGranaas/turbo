-- Upsert N50 named places + terrain points → anchors.anchor.
--
-- Two source tables in n50_staging:
--   stedsnavntekst : text labels with `geometri` POINT and a
--                    `navneobjekttype` classification (Norwegian).
--                    `streng` is the rendered name; `fulltekst` may
--                    be present for compound names.
--   terrengpunkt   : surveyed points with `hoyde` elevation and
--                    `posisjon` geometry. We import the high-hoyde
--                    ones as fallback summit candidates (filling in
--                    gaps where stedsnavntekst missed an unnamed top).
--
-- navneobjekttype mapping (Norwegian → AnchorKind):
--   Fjell / Fjelltopp / Topp / Berg / Nut / Tind / Pigg / Horn → summit
--   Hytte / Turisthytte / DnT-hytte / Selvbetjent hytte         → cabin
--   Innsjø / Vann / Tjern / Vatn                                → waterfeature
--   anything else                                               → named_place
--
-- terrengpunkt always becomes kind='summit' when hoyde > 600m
-- (cutoff filters out cairns and trig points at low elevations that
-- aren't meaningful destinations).

DELETE FROM anchors.anchor
WHERE source_ref LIKE 'n50-stedsnavn-%'
   OR source_ref LIKE 'n50-terrengpunkt-%';

INSERT INTO anchors.anchor
    (kind, geom, name, elevation_m, sources, source_ref, attrs)
SELECT
    -- N50's navneobjekttype values are lowercase + sometimes
    -- camelCased (e.g. `seterStøl`, `holmeISjø`). LOWER() for safety;
    -- list the variants we've actually seen in the real dataset.
    CASE LOWER(COALESCE(navneobjekttype, ''))
        -- Mountains, peaks, ridges
        WHEN 'fjell' THEN 'summit'
        WHEN 'fjelltopp' THEN 'summit'
        WHEN 'topp' THEN 'summit'
        WHEN 'berg' THEN 'summit'
        WHEN 'nut' THEN 'summit'
        WHEN 'tind' THEN 'summit'
        WHEN 'pigg' THEN 'summit'
        WHEN 'horn' THEN 'summit'
        WHEN 'haug' THEN 'summit'
        WHEN 'ås' THEN 'summit'
        -- Cabins / huts (DNT + private)
        WHEN 'hytte' THEN 'cabin'
        WHEN 'turisthytte' THEN 'cabin'
        WHEN 'dnt-hytte' THEN 'cabin'
        WHEN 'selvbetjent hytte' THEN 'cabin'
        WHEN 'ubetjent hytte' THEN 'cabin'
        WHEN 'seterstøl' THEN 'cabin'
        -- Water bodies
        WHEN 'innsjø' THEN 'waterfeature'
        WHEN 'vann' THEN 'waterfeature'
        WHEN 'tjern' THEN 'waterfeature'
        WHEN 'vatn' THEN 'waterfeature'
        ELSE 'named_place'
    END,
    -- N50 stores stedsnavntekst.geometri as a LineString (the text
    -- baseline for label rendering on the map). Extract the start
    -- point — close enough to the actual feature for anchor purposes.
    -- For genuine Points (older N50 vintages), ST_StartPoint passes
    -- through unchanged.
    CASE ST_GeometryType(geometri)
        WHEN 'ST_LineString' THEN ST_StartPoint(geometri)
        WHEN 'ST_MultiLineString' THEN ST_StartPoint(ST_GeometryN(geometri, 1))
        WHEN 'ST_Point' THEN geometri
        WHEN 'ST_MultiPoint' THEN ST_GeometryN(geometri, 1)
        ELSE NULL
    END::geometry(Point, 25833) AS geom,
    NULLIF(COALESCE(streng, fulltekst), ''),
    NULL::float8,
    ARRAY['n50']::text[],
    'n50-stedsnavn-' || objid::text,
    jsonb_build_object('navneobjekttype', navneobjekttype,
                       'navneobjektgruppe', navneobjektgruppe)
FROM n50_staging.stedsnavntekst
WHERE geometri IS NOT NULL
-- Partial unique index on (source_ref WHERE source_ref IS NOT NULL).
-- Predicate must match the index for ON CONFLICT inference to work.
ON CONFLICT (source_ref) WHERE source_ref IS NOT NULL DO NOTHING;

-- High terrengpunkt as fallback summit anchors.
INSERT INTO anchors.anchor
    (kind, geom, name, elevation_m, sources, source_ref, attrs)
SELECT
    'summit',
    posisjon::geometry(Point, 25833),
    NULL::text,
    hoyde::float8,
    ARRAY['n50']::text[],
    'n50-terrengpunkt-' || objid::text,
    jsonb_build_object('source_table', 'terrengpunkt')
FROM n50_staging.terrengpunkt
WHERE posisjon IS NOT NULL
  AND hoyde IS NOT NULL
  AND hoyde > 600
ON CONFLICT (source_ref) WHERE source_ref IS NOT NULL DO NOTHING;

-- Snap to graph. Bound to 2000 m; anchors farther than that don't get
-- a snap and become un-routable (correct behaviour — Theta* off-trail
-- can still find a path if the user opts in).
UPDATE anchors.anchor a
SET snapped_node_id = nn.node_id, snap_distance_m = nn.dist
FROM (
    SELECT a2.id AS aid, n.id AS node_id,
           ST_Distance(n.geom, a2.geom) AS dist,
           ROW_NUMBER() OVER (
               PARTITION BY a2.id ORDER BY ST_Distance(n.geom, a2.geom)
           ) AS rn
    FROM anchors.anchor a2
    JOIN paths.node n ON ST_DWithin(n.geom, a2.geom, 2000.0)
    WHERE a2.source_ref LIKE 'n50-%' AND a2.snapped_node_id IS NULL
) nn
WHERE nn.aid = a.id AND nn.rn = 1;
