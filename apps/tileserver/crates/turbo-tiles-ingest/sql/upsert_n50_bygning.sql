-- Upsert N50 building footprints (bygning_omrade) → terrain.building_polygon.
-- Sources after pgdump_load::restore: schema `n50_staging`.
--
-- Source table shape (from the dump):
--   n50_staging.bygning_omrade (objid, objtype, omrade GEOMETRY(_,25833),
--                               bygningstype, betjeningsgrad, hytteeier,
--                               tilgjengelighet, navn, ...)
--
-- Geometry column is `omrade`; `bygningstype` is the N50 building class
-- (numeric code as text), `navn` is an optional name (e.g. cabins).

DELETE FROM terrain.building_polygon WHERE source = 'n50';

INSERT INTO terrain.building_polygon (geom, building_type, name, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(ST_CurveToLine(s.omrade)), 3)),
    NULLIF(s.bygningstype, ''),
    NULLIF(s.navn, ''),
    'n50',
    jsonb_build_object(
        'objtype', s.objtype,
        'betjeningsgrad', s.betjeningsgrad
    )
FROM n50_staging.bygning_omrade s
WHERE s.omrade IS NOT NULL
  AND NOT ST_IsEmpty(s.omrade)
  -- Drop degenerate slivers that survive MakeValid as empty collections.
  AND NOT ST_IsEmpty(ST_CollectionExtract(ST_MakeValid(ST_CurveToLine(s.omrade)), 3));
