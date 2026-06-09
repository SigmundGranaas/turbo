-- Upsert N50 coastline (kystkontur) → terrain.coastline.
-- Sources after pgdump_load::restore: schema `n50_staging`.
--
-- Source table shape (from the dump):
--   n50_staging.kystkontur (objid, objtype, grense GEOMETRY(_,25833),
--                           datafangstdato, oppdateringsdato, malemetode,
--                           noyaktighet)
--
-- Geometry column is `grense` (the boundary line). Typed GEOMETRY; ST_Dump
-- explodes any MultiLineString parts so the canonical column stays a clean
-- LineString and nothing is silently dropped.

DELETE FROM terrain.coastline WHERE source = 'n50';

INSERT INTO terrain.coastline (geom, source, attrs)
SELECT
    (ST_Dump(s.grense)).geom::geometry(LineString, 25833),
    'n50',
    jsonb_build_object('objtype', s.objtype)
FROM n50_staging.kystkontur s
WHERE s.grense IS NOT NULL
  AND NOT ST_IsEmpty(s.grense);
