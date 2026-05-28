-- Upsert Turrutebasen → paths.edge + trails.trail + trails.trail_edge.
-- Source: turbase_staging schema after pgdump_load::restore.
--
-- Confirmed table shape (from the Friluftsliv dump):
--   fotrute (objid, objtype, skilting, anleggsnummer, uukoblingsid,
--            belysning, senterlinje GEOMETRY(_,25833),
--            lokalid, navnerom, versjonid,
--            datafangstdato, oppdateringsdato, noyaktighet, opphav,
--            omradeid, originaldatavert, kopidato,
--            informasjon, merking, rutefolger, underlagstype,
--            rutebredde, trafikkbelastning, sesong, malemetode)
--
-- merking values are text codes from rutemerking lookup ('Rød', 'Blå', etc.)
-- sesong values are text codes from sesong lookup ('Sommer', 'Vinter', 'Hele året').

-- Clear prior turbase rows.
DELETE FROM trails.trail_edge WHERE trail_id IN (
    SELECT id FROM trails.trail WHERE source = 'turbase'
);
DELETE FROM trails.trail WHERE source = 'turbase';
DELETE FROM paths.edge
WHERE deleted_at IS NULL AND ingest_source = 'turbase';

-- Phase 1: edges from foot trails.
INSERT INTO paths.edge
    (geom, fkb_type, marking, season, attrs, attr_hash, ingest_source)
SELECT
    ST_Force2D(s.senterlinje)::geometry(LineString, 25833),
    'sti'::text,
    CASE LOWER(COALESCE(s.merking, ''))
        WHEN 'rød' THEN 'red'
        WHEN 'rod' THEN 'red'
        WHEN 'blå' THEN 'blue'
        WHEN 'bla' THEN 'blue'
        WHEN 'sort' THEN 'black'
        WHEN 'svart' THEN 'black'
        WHEN 't-merket' THEN 't'
        ELSE NULL
    END,
    CASE LOWER(COALESCE(s.sesong, 'sommer'))
        WHEN 'vinter' THEN ARRAY['winter']::text[]
        WHEN 'hele året' THEN ARRAY['summer','winter']::text[]
        WHEN 'hele aret' THEN ARRAY['summer','winter']::text[]
        ELSE ARRAY['summer']::text[]
    END,
    jsonb_build_object(
        'lokalid', s.lokalid,
        'objid', s.objid,
        'rutefolger', s.rutefolger,
        'rutebredde', s.rutebredde,
        'underlagstype', s.underlagstype,
        'trafikkbelastning', s.trafikkbelastning
    ),
    encode(sha256(('turbase-fotrute-' || s.objid::text)::bytea), 'hex'),
    'turbase'::paths.ingest_source
FROM turbase_staging.fotrute s
WHERE s.senterlinje IS NOT NULL
  AND NOT ST_IsEmpty(s.senterlinje)
  AND ST_GeometryType(s.senterlinje) IN ('ST_LineString', 'ST_MultiLineString')
ON CONFLICT (attr_hash) WHERE deleted_at IS NULL DO NOTHING;

-- Phase 1b: ski tracks. fkb_type='skiloype' so the ski profile cost
-- expression picks them with a heavy preference.
INSERT INTO paths.edge
    (geom, fkb_type, marking, season, attrs, attr_hash, ingest_source)
SELECT
    ST_Force2D(s.senterlinje)::geometry(LineString, 25833),
    'skiloype'::text,
    NULL::text,
    ARRAY['winter']::text[],
    jsonb_build_object(
        'lokalid', s.lokalid,
        'objid', s.objid
    ),
    encode(sha256(('turbase-skiloype-' || s.objid::text)::bytea), 'hex'),
    'turbase'::paths.ingest_source
FROM turbase_staging.skiloype s
WHERE s.senterlinje IS NOT NULL
  AND NOT ST_IsEmpty(s.senterlinje)
  AND ST_GeometryType(s.senterlinje) IN ('ST_LineString', 'ST_MultiLineString')
ON CONFLICT (attr_hash) WHERE deleted_at IS NULL DO NOTHING;

-- Phase 2: trail rollups. One trail per anleggsnummer (the upstream
-- "trail-id" — many edges share it). Where anleggsnummer is missing,
-- group by lokalid so every edge ends up in some rollup.
INSERT INTO trails.trail
    (name, operator, mark_colour, source, source_ref, season, attrs)
SELECT DISTINCT
    NULL::text AS name,
    NULL::text AS operator,
    CASE LOWER(COALESCE(s.merking, ''))
        WHEN 'rød' THEN 'red'
        WHEN 'rod' THEN 'red'
        WHEN 'blå' THEN 'blue'
        WHEN 'bla' THEN 'blue'
        WHEN 'sort' THEN 'black'
        WHEN 'svart' THEN 'black'
        WHEN 't-merket' THEN 't'
        ELSE NULL
    END,
    'turbase',
    'turbase-' || COALESCE(s.anleggsnummer, s.lokalid),
    CASE LOWER(COALESCE(s.sesong, 'sommer'))
        WHEN 'vinter' THEN ARRAY['winter']::text[]
        WHEN 'hele året' THEN ARRAY['summer','winter']::text[]
        ELSE ARRAY['summer']::text[]
    END,
    jsonb_build_object('rutebredde', s.rutebredde)
FROM turbase_staging.fotrute s
WHERE COALESCE(s.anleggsnummer, s.lokalid) IS NOT NULL;

-- Phase 3: trail → edge links.
INSERT INTO trails.trail_edge (trail_id, edge_id, seq)
SELECT t.id, e.id, ROW_NUMBER() OVER (
    PARTITION BY t.id ORDER BY e.id
)::int
FROM turbase_staging.fotrute s
JOIN trails.trail t
    ON t.source = 'turbase'
   AND t.source_ref = 'turbase-' || COALESCE(s.anleggsnummer, s.lokalid)
JOIN paths.edge e
    ON e.ingest_source = 'turbase'
   AND e.attrs->>'objid' = s.objid::text
ON CONFLICT (trail_id, edge_id) DO NOTHING;
