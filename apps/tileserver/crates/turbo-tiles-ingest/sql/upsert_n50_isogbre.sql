-- Upsert N50 snoisbre (glaciers + perpetual snow) → terrain.glacier_polygon.
-- One table covers both ice and snow in N50; we don't separate.

DELETE FROM terrain.glacier_polygon WHERE source = 'n50';

INSERT INTO terrain.glacier_polygon (geom, name, source, attrs)
SELECT
    ST_Multi(ST_CollectionExtract(ST_MakeValid(s.omrade), 3)),
    NULL::text,
    'n50',
    jsonb_build_object('objtype', s.objtype)
FROM n50_staging.snoisbre s
WHERE s.omrade IS NOT NULL AND NOT ST_IsEmpty(s.omrade);
