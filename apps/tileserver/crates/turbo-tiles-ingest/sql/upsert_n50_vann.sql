-- Upsert N50 lakes (innsjo) + sea (havflate) → terrain.water_polygon.
-- Sources after pgdump_load::restore: schema `n50_staging`.
--
-- Source table shape (from the dump):
--   n50_staging.innsjo (objid, objtype, omrade GEOMETRY(_,25833),
--                       vatnlopenummer, hoyde, oppdateringsdato)
--   n50_staging.havflate (objid, objtype, omrade GEOMETRY(_,25833),
--                         oppdateringsdato)
--
-- Geometry column is `omrade` (Norwegian for "area"), not `geom`.

DELETE FROM terrain.water_polygon WHERE source = 'n50';

INSERT INTO terrain.water_polygon (geom, name, kind, elev_m, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(s.omrade), 3)),
    NULL::text,
    'lake'::text,
    s.hoyde::float8,
    'n50',
    jsonb_build_object('vatnlopenummer', s.vatnlopenummer, 'objtype', s.objtype)
FROM n50_staging.innsjo s
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);

INSERT INTO terrain.water_polygon (geom, name, kind, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(s.omrade), 3)),
    NULL::text,
    'sea'::text,
    'n50',
    jsonb_build_object('objtype', s.objtype)
FROM n50_staging.havflate s
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);
