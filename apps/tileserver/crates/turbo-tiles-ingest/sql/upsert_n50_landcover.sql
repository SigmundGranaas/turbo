-- Upsert N50 landcover-equivalent polygons → terrain.landcover_patch.
-- Source tables in n50_staging:
--   skog         → 'forest'
--   myr          → 'wetland'
--   apentomrade  → 'open'
--   dyrketmark   → 'open' (cultivated counts as open for hiking)
--
-- Each source table has the same shape: (objid, objtype, omrade, oppdateringsdato).
-- N50 already provides everything AR50 would have for the off-trail
-- cost model; we keep AR50 schema names in the canonical table so the
-- runtime is source-agnostic.

DELETE FROM terrain.landcover_patch WHERE source = 'n50';

INSERT INTO terrain.landcover_patch (geom, class, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(omrade), 3)),
    'forest',
    'n50',
    jsonb_build_object('source_table', 'skog', 'objtype', objtype)
FROM n50_staging.skog
WHERE omrade IS NOT NULL AND NOT ST_IsEmpty(omrade);

INSERT INTO terrain.landcover_patch (geom, class, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(omrade), 3)),
    'wetland',
    'n50',
    jsonb_build_object('source_table', 'myr', 'objtype', objtype)
FROM n50_staging.myr
WHERE omrade IS NOT NULL AND NOT ST_IsEmpty(omrade);

INSERT INTO terrain.landcover_patch (geom, class, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(omrade), 3)),
    'open',
    'n50',
    jsonb_build_object('source_table', 'apentomrade', 'objtype', objtype)
FROM n50_staging.apentomrade
WHERE omrade IS NOT NULL AND NOT ST_IsEmpty(omrade);

INSERT INTO terrain.landcover_patch (geom, class, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(omrade), 3)),
    'open',
    'n50',
    jsonb_build_object('source_table', 'dyrketmark', 'objtype', objtype)
FROM n50_staging.dyrketmark
WHERE omrade IS NOT NULL AND NOT ST_IsEmpty(omrade);
