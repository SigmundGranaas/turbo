-- Upsert N50 vegnett (veglenke) → paths.edge.
--
-- IMPORTANT: in the N50 dump, `objid` is NOT globally unique. It
-- restarts at 0 *per typeveg category*, so the same objid appears
-- once each for {enkelBilveg, sti, traktorveg, gangOgSykkelveg,
-- bilferje, passasjerferje, barmarksløype}. The old hash of
-- `'n50_vegnett-' || objid` collided across all 7 categories and
-- the ON CONFLICT skipped every non-first category. Concretely:
-- 179,315 sti rows + 121,923 traktorveg + 16,590 gangsykkel were
-- silently dropped on every restore, leaving the routing graph
-- with roads only. The hash now includes `typeveg`.
--
-- The CASE mapping also gains a `sti` branch (was missing) and a
-- `barmarksløype` branch. Strings match what the graph builder's
-- `encode_fkb_type` knows about so the bytes land in the right
-- per-edge code: sti=1, vei=2, skiloype=3. Unknown spellings
-- collapse to `vei` at the runtime encoder, which is fine for
-- ferries / unmapped categories.

DELETE FROM paths.edge
WHERE deleted_at IS NULL
  AND ingest_source = 'fkb'
  AND attrs->>'source' = 'n50_vegnett';

INSERT INTO paths.edge
    (geom, fkb_type, season, attrs, attr_hash, ingest_source)
SELECT
    ST_Force2D(s.senterlinje)::geometry(LineString, 25833),
    CASE
        WHEN s.typeveg ILIKE 'sti%'         THEN 'sti'
        WHEN s.typeveg ILIKE 'traktorveg%'  THEN 'traktorvei'
        WHEN s.typeveg ILIKE 'skogsbilveg%' THEN 'skogsvei'
        WHEN s.typeveg ILIKE 'gang%' OR s.typeveg ILIKE 'sykkel%' THEN 'sti'
        WHEN s.typeveg ILIKE 'barmark%'     THEN 'sti'
        ELSE 'vei'
    END,
    ARRAY['summer','winter']::text[],
    jsonb_build_object('source', 'n50_vegnett',
                       'typeveg', s.typeveg,
                       'vegkategori', s.vegkategori,
                       'objid', s.objid),
    encode(
        sha256(('n50_vegnett-' || COALESCE(s.typeveg,'') || '-' || s.objid::text)::bytea),
        'hex'
    ),
    'fkb'::paths.ingest_source
FROM n50_staging.veglenke s
WHERE s.senterlinje IS NOT NULL
  AND NOT ST_IsEmpty(s.senterlinje)
  AND ST_GeometryType(s.senterlinje) IN ('ST_LineString', 'ST_MultiLineString')
ON CONFLICT (attr_hash) WHERE deleted_at IS NULL DO NOTHING;
